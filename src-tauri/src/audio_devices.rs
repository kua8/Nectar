use serde::Serialize;
use windows::core::{IUnknown, Interface, GUID, HRESULT, PCWSTR};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    eCommunications, eConsole, eMultimedia, eRender, ERole, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ};

const CPOLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
const IID_POLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
const PKEY_DEVICE_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

#[derive(Serialize)]
pub struct AudioDevice {
    id: String,
    name: String,
    is_default: bool,
}

unsafe fn device_id(device: &IMMDevice) -> Option<String> {
    let raw = device.GetId().ok()?;
    let id = String::from_utf16_lossy(raw.as_wide());
    CoTaskMemFree(Some(raw.0 as *const _));
    Some(id)
}

unsafe fn device_name(device: &IMMDevice) -> String {
    let Ok(store) = device.OpenPropertyStore(STGM_READ) else { return "Audio device".into() };
    let Ok(mut value) = store.GetValue(&PKEY_DEVICE_FRIENDLY_NAME) else { return "Audio device".into() };
    let name = PropVariantToStringAlloc(&value)
        .ok()
        .map(|s| String::from_utf16_lossy(s.as_wide()))
        .unwrap_or_else(|| "Audio device".into());
    let _ = PropVariantClear(&mut value);
    name
}

fn list_devices() -> Result<Vec<AudioDevice>, String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let default = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .ok()
            .and_then(|d| device_id(&d));
        let devices = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for i in 0..devices.GetCount().map_err(|e| e.to_string())? {
            let Ok(device) = devices.Item(i) else { continue };
            let Some(id) = device_id(&device) else { continue };
            out.push(AudioDevice { is_default: default.as_deref() == Some(id.as_str()), name: device_name(&device), id });
        }
        Ok(out)
    }
}

fn set_default(id: &str) -> Result<(), String> {
    type SetDefaultEndpoint = unsafe extern "system" fn(*mut std::ffi::c_void, PCWSTR, ERole) -> HRESULT;
    type Release = unsafe extern "system" fn(*mut std::ffi::c_void) -> u32;
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let unknown: IUnknown = CoCreateInstance(&CPOLICY_CONFIG_CLIENT, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let mut raw: *mut std::ffi::c_void = std::ptr::null_mut();
        unknown.query(&IID_POLICY_CONFIG, &mut raw).ok().map_err(|e| e.to_string())?;
        let table = *(raw as *const *const usize);
        let set_default_endpoint: SetDefaultEndpoint = std::mem::transmute(*table.add(13));
        let release: Release = std::mem::transmute(*table.add(2));
        let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
        let mut result = Ok(());
        for role in [eConsole, eMultimedia, eCommunications] {
            if let Err(e) = set_default_endpoint(raw, PCWSTR(wide.as_ptr()), role).ok() {
                result = Err(e.to_string());
                break;
            }
        }
        release(raw);
        result
    }
}

#[tauri::command]
pub async fn get_audio_devices() -> Result<Vec<AudioDevice>, String> {
    tauri::async_runtime::spawn_blocking(list_devices).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_default_audio_device(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || set_default(&id)).await.map_err(|e| e.to_string())?
}



