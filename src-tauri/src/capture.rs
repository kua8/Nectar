use std::time::Duration;
use windows::core::Interface;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

pub fn capture_window_bgra(hwnd: HWND) -> Option<(Vec<u8>, u32, u32)> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        if !GraphicsCaptureSession::IsSupported().ok()? {
            return None;
        }
        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().ok()?;
        let item: GraphicsCaptureItem = interop.CreateForWindow(hwnd).ok()?;
        let size = item.Size().ok()?;
        if size.Width < 16 || size.Height < 16 {
            return None;
        }

        let mut device = None;
        let mut context = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .ok()?;
        let device: ID3D11Device = device?;
        let context: ID3D11DeviceContext = context?;
        let dxgi: IDXGIDevice = device.cast().ok()?;
        let d3d: IDirect3DDevice = CreateDirect3D11DeviceFromDXGIDevice(&dxgi).ok()?.cast().ok()?;

        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&d3d, DirectXPixelFormat::B8G8R8A8UIntNormalized, 1, size).ok()?;
        let session = pool.CreateCaptureSession(&item).ok()?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        let _ = session.SetIsBorderRequired(false);
        session.StartCapture().ok()?;

        let mut frame = None;
        for _ in 0..25 {
            if let Ok(f) = pool.TryGetNextFrame() {
                frame = Some(f);
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let result = frame.and_then(|frame| {
            let content = frame.ContentSize().ok()?;
            let texture: ID3D11Texture2D = frame.Surface().ok()?.cast::<IDirect3DDxgiInterfaceAccess>().ok()?.GetInterface().ok()?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc);
            desc.Usage = D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
            desc.MiscFlags = 0;
            let mut staging = None;
            device.CreateTexture2D(&desc, None, Some(&mut staging)).ok()?;
            let staging = staging?;
            context.CopyResource(&staging, &texture);

            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).ok()?;
            let width = (content.Width.max(0) as u32).min(desc.Width);
            let height = (content.Height.max(0) as u32).min(desc.Height);
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for row in 0..height as usize {
                let src = std::slice::from_raw_parts((mapped.pData as *const u8).add(row * mapped.RowPitch as usize), width as usize * 4);
                out.extend_from_slice(src);
            }
            context.Unmap(&staging, 0);
            Some((out, width, height))
        });
        let _ = session.Close();
        let _ = pool.Close();
        result
    }
}
