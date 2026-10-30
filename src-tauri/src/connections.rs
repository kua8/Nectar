use serde::Serialize;
use windows::core::{GUID, HSTRING, PCWSTR};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::WiFi::{
    dot11_BSS_type_infrastructure, wlan_connection_mode_profile, WlanCloseHandle, WlanConnect, WlanDeleteProfile,
    WlanDisconnect, WlanEnumInterfaces, WlanFreeMemory, WlanGetAvailableNetworkList, WlanOpenHandle, WlanScan,
    WlanSetProfile, DOT11_SSID, WLAN_AVAILABLE_NETWORK_LIST, WLAN_CONNECTION_PARAMETERS, WLAN_INTERFACE_INFO_LIST,
};

const NETWORK_CONNECTED: u32 = 0x1;
const NETWORK_HAS_PROFILE: u32 = 0x2;

#[derive(Serialize, Clone)]
pub struct WifiNetwork {
    ssid: String,
    signal: u32,
    secured: bool,
    connected: bool,
    known: bool,
}

#[derive(Serialize)]
pub struct BluetoothDeviceInfo {
    id: String,
    name: String,
    connected: bool,
}

struct Wlan {
    handle: HANDLE,
    interface: GUID,
}

impl Wlan {
    fn open() -> Result<Self, String> {
        unsafe {
            let mut negotiated = 0u32;
            let mut handle = HANDLE::default();
            if WlanOpenHandle(2, None, &mut negotiated, &mut handle) != 0 {
                return Err("Wi-Fi is not available on this PC".into());
            }
            let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
            let status = WlanEnumInterfaces(handle, None, &mut list);
            let interface = if status == 0 && !list.is_null() && (*list).dwNumberOfItems > 0 {
                Some((*list).InterfaceInfo[0].InterfaceGuid)
            } else {
                None
            };
            if !list.is_null() {
                WlanFreeMemory(list as *const _);
            }
            match interface {
                Some(interface) => Ok(Wlan { handle, interface }),
                None => {
                    WlanCloseHandle(handle, None);
                    Err("No Wi-Fi adapter found".into())
                }
            }
        }
    }
}

impl Drop for Wlan {
    fn drop(&mut self) {
        unsafe { WlanCloseHandle(self.handle, None) };
    }
}

fn ssid_to_string(ssid: &DOT11_SSID) -> String {
    let len = (ssid.uSSIDLength as usize).min(ssid.ucSSID.len());
    String::from_utf8_lossy(&ssid.ucSSID[..len]).into_owned()
}

fn ssid_from_str(name: &str) -> Result<DOT11_SSID, String> {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 32 {
        return Err("Invalid network name".into());
    }
    let mut ssid = DOT11_SSID { uSSIDLength: bytes.len() as u32, ucSSID: [0; 32] };
    ssid.ucSSID[..bytes.len()].copy_from_slice(bytes);
    Ok(ssid)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn profile_xml(ssid: &str, password: Option<&str>) -> String {
    let name = xml_escape(ssid);
    let security = match password {
        Some(key) => format!(
            "<authEncryption><authentication>WPA2PSK</authentication><encryption>AES</encryption><useOneX>false</useOneX></authEncryption>\
             <sharedKey><keyType>passPhrase</keyType><protected>false</protected><keyMaterial>{}</keyMaterial></sharedKey>",
            xml_escape(key)
        ),
        None => "<authEncryption><authentication>open</authentication><encryption>none</encryption><useOneX>false</useOneX></authEncryption>".to_string(),
    };
    format!(
        "<?xml version=\"1.0\"?><WLANProfile xmlns=\"http://www.microsoft.com/networking/WLAN/profile/v1\">\
         <name>{name}</name><SSIDConfig><SSID><name>{name}</name></SSID></SSIDConfig>\
         <connectionType>ESS</connectionType><connectionMode>auto</connectionMode>\
         <MSM><security>{security}</security></MSM></WLANProfile>"
    )
}

fn scan_networks() -> Result<Vec<WifiNetwork>, String> {
    let wlan = Wlan::open()?;
    unsafe {
        let _ = WlanScan(wlan.handle, &wlan.interface, None, None, None);
        let mut list: *mut WLAN_AVAILABLE_NETWORK_LIST = std::ptr::null_mut();
        if WlanGetAvailableNetworkList(wlan.handle, &wlan.interface, 0, None, &mut list) != 0 || list.is_null() {
            return Err("Could not read the network list".into());
        }
        let entries = std::slice::from_raw_parts((*list).Network.as_ptr(), (*list).dwNumberOfItems as usize);
        let mut networks: Vec<WifiNetwork> = Vec::new();
        for entry in entries {
            let ssid = ssid_to_string(&entry.dot11Ssid);
            if ssid.is_empty() {
                continue;
            }
            let network = WifiNetwork {
                ssid,
                signal: entry.wlanSignalQuality,
                secured: entry.bSecurityEnabled.as_bool(),
                connected: entry.dwFlags & NETWORK_CONNECTED != 0,
                known: entry.dwFlags & NETWORK_HAS_PROFILE != 0,
            };
            match networks.iter_mut().find(|n| n.ssid == network.ssid) {
                Some(existing) => {
                    existing.connected |= network.connected;
                    existing.known |= network.known;
                    existing.signal = existing.signal.max(network.signal);
                }
                None => networks.push(network),
            }
        }
        WlanFreeMemory(list as *const _);
        networks.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.known.cmp(&a.known)).then(b.signal.cmp(&a.signal)));
        Ok(networks)
    }
}

fn connect_network(ssid: &str, password: Option<&str>) -> Result<(), String> {
    let wlan = Wlan::open()?;
    let mut dot11 = ssid_from_str(ssid)?;
    let known = scan_networks()?.iter().any(|n| n.ssid == ssid && n.known);
    unsafe {
        let profile_name = wide(ssid);
        if !known || password.is_some() {
            let secured = password.is_some();
            let xml = wide(&profile_xml(ssid, if secured { password } else { None }));
            let mut reason = 0u32;
            let status = WlanSetProfile(wlan.handle, &wlan.interface, 0, PCWSTR(xml.as_ptr()), None, true, None, &mut reason);
            if status != 0 {
                return Err("Windows rejected this network's settings".into());
            }
        }
        let params = WLAN_CONNECTION_PARAMETERS {
            wlanConnectionMode: wlan_connection_mode_profile,
            strProfile: PCWSTR(profile_name.as_ptr()),
            pDot11Ssid: &mut dot11,
            pDesiredBssidList: std::ptr::null_mut(),
            dot11BssType: dot11_BSS_type_infrastructure,
            dwFlags: 0,
        };
        if WlanConnect(wlan.handle, &wlan.interface, &params, None) != 0 {
            return Err("Could not start the connection".into());
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn wifi_scan() -> Result<Vec<WifiNetwork>, String> {
    tauri::async_runtime::spawn_blocking(scan_networks).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn wifi_connect(ssid: String, password: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || connect_network(&ssid, password.as_deref().filter(|p| !p.is_empty())))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn wifi_disconnect() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        let wlan = Wlan::open()?;
        if unsafe { WlanDisconnect(wlan.handle, &wlan.interface, None) } != 0 {
            return Err("Could not disconnect".to_string());
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn wifi_forget(ssid: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let wlan = Wlan::open()?;
        let name = wide(&ssid);
        if unsafe { WlanDeleteProfile(wlan.handle, &wlan.interface, PCWSTR(name.as_ptr()), None) } != 0 {
            return Err("Could not forget this network".to_string());
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
pub struct LanStatus {
    present: bool,
    connected: bool,
    name: String,
}

fn lan_status_sync() -> LanStatus {
    use windows::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
        GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    const IF_TYPE_ETHERNET: u32 = 6;
    const VIRTUAL_MARKERS: [&str; 10] =
        ["virtual", "vmware", "hyper-v", "vethernet", "vpn", "tap-", "tunnel", "loopback", "docker", "bluetooth"];
    let none = LanStatus { present: false, connected: false, name: String::new() };
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER | GAA_FLAG_INCLUDE_GATEWAYS;

    unsafe {
        let mut size = 0u32;
        GetAdaptersAddresses(0, flags, None, None, &mut size);
        if size == 0 {
            return none;
        }
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let first = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        if GetAdaptersAddresses(0, flags, None, Some(first), &mut size) != 0 {
            return none;
        }

        let mut status = none;
        let mut adapter = first as *const IP_ADAPTER_ADDRESSES_LH;
        while !adapter.is_null() {
            let a = &*adapter;
            let description = a.Description.to_string().unwrap_or_default();
            let lowered = description.to_lowercase();
            if a.IfType == IF_TYPE_ETHERNET
                && a.PhysicalAddressLength > 0
                && !VIRTUAL_MARKERS.iter().any(|marker| lowered.contains(marker))
            {
                status.present = true;
                let has_route = !a.FirstGatewayAddress.is_null();
                if a.OperStatus == IfOperStatusUp && has_route && !status.connected {
                    status.connected = true;
                    status.name = a.FriendlyName.to_string().unwrap_or(description);
                }
            }
            adapter = a.Next;
        }
        status
    }
}

#[tauri::command]
pub async fn get_lan_status() -> LanStatus {
    tauri::async_runtime::spawn_blocking(lan_status_sync)
        .await
        .unwrap_or(LanStatus { present: false, connected: false, name: String::new() })
}

#[tauri::command]
pub fn open_ethernet_settings() {
    use windows::Win32::UI::Shell::ShellExecuteA;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    unsafe {
        let _ = ShellExecuteA(
            None,
            windows::core::PCSTR(c"open".as_ptr() as *const u8),
            windows::core::PCSTR(c"ms-settings:network-ethernet".as_ptr() as *const u8),
            windows::core::PCSTR::null(),
            windows::core::PCSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

fn paired_bluetooth_devices() -> Result<Vec<BluetoothDeviceInfo>, String> {
    use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothDevice};
    use windows::Devices::Enumeration::DeviceInformation;
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
    }
    let selector = BluetoothDevice::GetDeviceSelectorFromPairingState(true).map_err(|e| e.to_string())?;
    let infos = DeviceInformation::FindAllAsyncAqsFilter(&selector)
        .and_then(|op| op.get())
        .map_err(|e| e.to_string())?;
    let mut devices = Vec::new();
    for i in 0..infos.Size().unwrap_or(0) {
        let Ok(info) = infos.GetAt(i) else { continue };
        let (Ok(id), Ok(name)) = (info.Id(), info.Name()) else { continue };
        let connected = BluetoothDevice::FromIdAsync(&id)
            .and_then(|op| op.get())
            .and_then(|device| device.ConnectionStatus())
            .map(|status| status == BluetoothConnectionStatus::Connected)
            .unwrap_or(false);
        devices.push(BluetoothDeviceInfo { id: id.to_string(), name: name.to_string(), connected });
    }
    devices.sort_by(|a, b| b.connected.cmp(&a.connected).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(devices)
}

#[tauri::command]
pub async fn bluetooth_devices() -> Result<Vec<BluetoothDeviceInfo>, String> {
    tauri::async_runtime::spawn_blocking(paired_bluetooth_devices).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn bluetooth_unpair(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        use windows::Devices::Enumeration::{DeviceInformation, DeviceUnpairingResultStatus};
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
        }
        let info = DeviceInformation::CreateFromIdAsync(&HSTRING::from(id))
            .and_then(|op| op.get())
            .map_err(|e| e.to_string())?;
        let result = info
            .Pairing()
            .and_then(|pairing| pairing.UnpairAsync())
            .and_then(|op| op.get())
            .map_err(|e| e.to_string())?;
        match result.Status() {
            Ok(DeviceUnpairingResultStatus::Unpaired) | Ok(DeviceUnpairingResultStatus::AlreadyUnpaired) => Ok(()),
            _ => Err("Could not remove this device".to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
