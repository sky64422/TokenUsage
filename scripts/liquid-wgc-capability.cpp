// Default: capability probe only. --capture-test: two live capture sessions.
// Never changes window display affinity. --control omits the exclusion list.
#include <windows.h>
#include <d3d11.h>
#include <dxgi.h>
#include <windows.graphics.capture.interop.h>
#include <windows.graphics.directx.direct3d11.interop.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.UI.h>
#include <winrt/Windows.Graphics.Capture.h>
#include <winrt/Windows.Graphics.DirectX.h>
#include <winrt/Windows.Graphics.DirectX.Direct3D11.h>
#include <iostream>
#include <iomanip>
#include <chrono>
#include <string_view>
#include <set>

using namespace winrt;
using namespace winrt::Windows::Graphics::Capture;

// ABI verified against Microsoft's windows-rs Capture/mod.rs (2026-11-01),
// IDisplayGraphicsCaptureSession_Vtbl: IInspectable, Set, then Get.
// Commit: 317646c941aa18916ffa5daeb4ffd5ffa4cd26f3.
struct __declspec(uuid("bb91f61b-218a-587d-8580-2701a74c0525")) DisplaySession : IInspectable {
    virtual HRESULT __stdcall SetWindowExclusionList(void*, UINT64*) = 0;
    virtual HRESULT __stdcall GetWindowExclusionList(void**) = 0;
};

struct TestWindows {
    HWND background{}, foreground{};
    ~TestWindows() {
        if (foreground) DestroyWindow(foreground);
        if (background) DestroyWindow(background);
    }
};
constexpr int sampleX = 180, sampleY = 180;
constexpr COLORREF marker = RGB(255, 0, 255);
constexpr COLORREF backdropRed = RGB(220, 30, 30);
constexpr COLORREF backdropBlue = RGB(30, 30, 220);
COLORREF backdrop = backdropRed;

LRESULT CALLBACK testWindowProc(HWND hwnd, UINT message, WPARAM wp, LPARAM lp) {
    if (message == WM_PAINT) {
        PAINTSTRUCT paint{};
        auto dc = BeginPaint(hwnd, &paint);
        auto brush = CreateSolidBrush(GetWindowLongPtrW(hwnd, GWLP_USERDATA) ? marker : backdrop);
        FillRect(dc, &paint.rcPaint, brush);
        DeleteObject(brush);
        EndPaint(hwnd, &paint);
        return 0;
    }
    return DefWindowProcW(hwnd, message, wp, lp);
}

COLORREF sampleFrame(Direct3D11CaptureFrame const& frame, ID3D11DeviceContext* context,
                    ID3D11Texture2D* staging) {
    auto access = frame.Surface().as<::Windows::Graphics::DirectX::Direct3D11::IDirect3DDxgiInterfaceAccess>();
    com_ptr<ID3D11Texture2D> texture;
    check_hresult(access->GetInterface(__uuidof(ID3D11Texture2D), texture.put_void()));
    D3D11_BOX box{sampleX, sampleY, 0, sampleX + 1, sampleY + 1, 1};
    context->CopySubresourceRegion(staging, 0, 0, 0, 0, texture.get(), 0, &box);
    D3D11_MAPPED_SUBRESOURCE mapped{};
    check_hresult(context->Map(staging, 0, D3D11_MAP_READ, 0, &mapped));
    auto pixel = static_cast<unsigned char*>(mapped.pData);
    auto color = RGB(pixel[2], pixel[1], pixel[0]);
    context->Unmap(staging, 0);
    return color;
}

int captureTest(bool control, ID3D11Device* device,
                winrt::Windows::Graphics::DirectX::Direct3D11::IDirect3DDevice const& captureDevice,
                GraphicsCaptureItem const& item) {
    SetProcessDPIAware();
    WNDCLASSW wc{};
    wc.lpfnWndProc = testWindowProc;
    wc.hInstance = GetModuleHandleW(nullptr);
    wc.lpszClassName = L"TokenUsageWgcIsolationProbe";
    if (!RegisterClassW(&wc)) check_hresult(HRESULT_FROM_WIN32(GetLastError()));
    TestWindows windows;
    constexpr DWORD style = WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW;
    windows.background = CreateWindowExW(style, wc.lpszClassName, L"WGC test background",
        WS_POPUP, 100, 100, 160, 160, nullptr, nullptr, wc.hInstance, nullptr);
    windows.foreground = CreateWindowExW(style, wc.lpszClassName, L"WGC test foreground",
        WS_POPUP, 140, 140, 80, 80, nullptr, nullptr, wc.hInstance, nullptr);
    if (!windows.background || !windows.foreground) check_hresult(HRESULT_FROM_WIN32(GetLastError()));
    SetWindowLongPtrW(windows.foreground, GWLP_USERDATA, 1);
    ShowWindow(windows.background, SW_SHOWNOACTIVATE);
    ShowWindow(windows.foreground, SW_SHOWNOACTIVATE);
    auto makePool = [&] {
        return Direct3D11CaptureFramePool::CreateFreeThreaded(captureDevice,
            winrt::Windows::Graphics::DirectX::DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, item.Size());
    };
    auto ownPool = makePool(), externalPool = makePool();
    auto own = ownPool.CreateCaptureSession(item), external = externalPool.CreateCaptureSession(item);
    auto filter = own.as<DisplaySession>();
    auto excluded = single_threaded_vector<winrt::Windows::UI::WindowId>();
    excluded.Append({reinterpret_cast<uint64_t>(windows.foreground)});
    UINT64 iteration{};
    auto iterable = excluded.as<winrt::Windows::Foundation::Collections::IIterable<winrt::Windows::UI::WindowId>>();
    if (!control) check_hresult(filter->SetWindowExclusionList(get_abi(iterable), &iteration));
    com_ptr<ID3D11DeviceContext> context;
    device->GetImmediateContext(context.put());
    D3D11_TEXTURE2D_DESC desc{};
    desc.Width = desc.Height = desc.MipLevels = desc.ArraySize = 1;
    desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
    desc.SampleDesc.Count = 1;
    desc.Usage = D3D11_USAGE_STAGING;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
    com_ptr<ID3D11Texture2D> staging;
    check_hresult(device->CreateTexture2D(&desc, nullptr, staging.put()));
    own.IsCursorCaptureEnabled(false);
    external.IsCursorCaptureEnabled(false);
    own.StartCapture();
    external.StartCapture();
    int ownFrames = 0, externalFrames = 0, ownBad = 0, externalBad = 0;
    int redFrames = 0, blueFrames = 0, gdiSamples = 0, gdiBad = 0, affinityBad = 0;
    constexpr auto warmup = std::chrono::milliseconds(500);
    constexpr auto duration = std::chrono::seconds(6);
    constexpr auto colorPeriod = std::chrono::milliseconds(100);
    auto start = std::chrono::steady_clock::now();
    long long previousStep = -1;
    while (std::chrono::steady_clock::now() - start < duration) {
        auto elapsed = std::chrono::steady_clock::now() - start;
        auto step = elapsed / colorPeriod;
        if (step != previousStep) {
            previousStep = step;
            backdrop = step % 2 ? backdropBlue : backdropRed;
            InvalidateRect(windows.background, nullptr, FALSE);
            UpdateWindow(windows.background);
            if (elapsed > warmup) {
                auto dc = GetDC(nullptr);
                if (!dc) check_hresult(HRESULT_FROM_WIN32(GetLastError()));
                auto color = GetPixel(dc, sampleX, sampleY);
                ReleaseDC(nullptr, dc);
                ++gdiSamples;
                gdiBad += color != marker;
            }
        }
        MSG message;
        while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        DWORD affinity{};
        if (!GetWindowDisplayAffinity(windows.foreground, &affinity))
            check_hresult(HRESULT_FROM_WIN32(GetLastError()));
        affinityBad += affinity != WDA_NONE;
        if (auto frame = ownPool.TryGetNextFrame()) {
            auto color = sampleFrame(frame, context.get(), staging.get());
            if (elapsed > warmup) {
                ++ownFrames;
                redFrames += color == backdropRed;
                blueFrames += color == backdropBlue;
                ownBad += color != backdropRed && color != backdropBlue;
            }
        }
        if (auto frame = externalPool.TryGetNextFrame()) {
            auto color = sampleFrame(frame, context.get(), staging.get());
            if (elapsed > warmup) { ++externalFrames; externalBad += color != marker; }
        }
        Sleep(5); // Native observation pacing, not a product unit test.
    }
    own.Close(); external.Close(); ownPool.Close(); externalPool.Close();
    bool passed = ownFrames >= 20 && externalFrames >= 20 && redFrames && blueFrames
        && gdiSamples >= 20 && !ownBad && !externalBad && !gdiBad && !affinityBad;
    std::cout << "{\"status\":\"" << (passed ? "isolation_passed" : "isolation_failed")
        << "\",\"own_frames\":" << ownFrames << ",\"own_bad\":" << ownBad
        << ",\"red_frames\":" << redFrames << ",\"blue_frames\":" << blueFrames
        << ",\"external_frames\":" << externalFrames << ",\"external_bad\":" << externalBad
        << ",\"gdi_samples\":" << gdiSamples << ",\"gdi_bad\":" << gdiBad
        << ",\"affinity_bad\":" << affinityBad << "}\n";
    return passed ? 0 : 1;
}

// Independent recording process: no excluded HWNDs and no affinity mutation.
// The test driver supplies a notch ROI over a solid, changing test background.
int observeRegion(ID3D11Device* device, Direct3D11CaptureFramePool const& pool,
                  GraphicsCaptureSession const& session, RECT region, int expectedFrames) {
    if (region.left < 0 || region.top < 0 || region.right <= region.left || region.bottom <= region.top || expectedFrames < 1)
        throw hresult_invalid_argument();
    auto width = static_cast<UINT>(region.right - region.left);
    auto height = static_cast<UINT>(region.bottom - region.top);
    if (width < 8 || height < 8 || static_cast<uint64_t>(width) * height > 1048576)
        throw hresult_invalid_argument();
    com_ptr<ID3D11DeviceContext> context;
    device->GetImmediateContext(context.put());
    D3D11_TEXTURE2D_DESC desc{};
    desc.Width = width; desc.Height = height; desc.MipLevels = desc.ArraySize = 1;
    desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM; desc.SampleDesc.Count = 1;
    desc.Usage = D3D11_USAGE_STAGING; desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
    com_ptr<ID3D11Texture2D> staging;
    check_hresult(device->CreateTexture2D(&desc, nullptr, staging.put()));
    session.IsCursorCaptureEnabled(false);
    session.StartCapture();
    int frames = 0, missing = 0;
    auto start = std::chrono::steady_clock::now();
    constexpr auto timeout = std::chrono::seconds(45);
    while (frames < expectedFrames && std::chrono::steady_clock::now() - start < timeout) {
        if (auto frame = pool.TryGetNextFrame()) {
            auto access = frame.Surface().as<::Windows::Graphics::DirectX::Direct3D11::IDirect3DDxgiInterfaceAccess>();
            com_ptr<ID3D11Texture2D> texture;
            check_hresult(access->GetInterface(__uuidof(ID3D11Texture2D), texture.put_void()));
            D3D11_TEXTURE2D_DESC source{}; texture->GetDesc(&source);
            if (static_cast<UINT>(region.right) > source.Width || static_cast<UINT>(region.bottom) > source.Height)
                throw hresult_invalid_argument();
            D3D11_BOX box{static_cast<UINT>(region.left),static_cast<UINT>(region.top),0,
                static_cast<UINT>(region.right),static_cast<UINT>(region.bottom),1};
            context->CopySubresourceRegion(staging.get(),0,0,0,0,texture.get(),0,&box);
            D3D11_MAPPED_SUBRESOURCE mapped{};
            check_hresult(context->Map(staging.get(),0,D3D11_MAP_READ,0,&mapped));
            std::set<COLORREF> colors;
            // Ignore the outer capture-indicator pixels; foreground must exist inside the ROI.
            for (UINT y=3; y<height-3 && colors.size()<=16; ++y) {
                auto row=static_cast<unsigned char*>(mapped.pData)+y*mapped.RowPitch;
                for (UINT x=3; x<width-3 && colors.size()<=16; ++x)
                    colors.insert(RGB(row[x*4+2],row[x*4+1],row[x*4]));
            }
            context->Unmap(staging.get(),0);
            ++frames;
            missing += colors.size() <= 16;
        }
        Sleep(5);
    }
    session.Close(); pool.Close();
    std::cout << "{\"observer_frames\":" << frames << ",\"missing\":" << missing
        << ",\"expected_frames\":" << expectedFrames << ",\"independent_process\":true}\n";
    return frames == expectedFrames && !missing ? 0 : 1;
}

int main(int argc, char** argv) {
    try {
        init_apartment(apartment_type::multi_threaded);
        if (!GraphicsCaptureSession::IsSupported()) {
            std::cout << "{\"status\":\"wgc_unavailable\"}\n";
            return 2;
        }
        com_ptr<ID3D11Device> device;
        check_hresult(D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, nullptr, 0, D3D11_SDK_VERSION,
            device.put(), nullptr, nullptr));
        com_ptr<IInspectable> inspectable;
        check_hresult(CreateDirect3D11DeviceFromDXGIDevice(
            device.as<IDXGIDevice>().get(), inspectable.put()));
        auto captureDevice = inspectable.as<winrt::Windows::Graphics::DirectX::Direct3D11::IDirect3DDevice>();
        auto factory = get_activation_factory<GraphicsCaptureItem, IGraphicsCaptureItemInterop>();
        GraphicsCaptureItem item{nullptr};
        bool observing = argc == 7 && std::string_view(argv[1]) == "--observe";
        POINT origin{observing ? std::stoi(argv[2]) : 0, observing ? std::stoi(argv[3]) : 0};
        auto monitor = MonitorFromPoint(origin, MONITOR_DEFAULTTOPRIMARY);
        check_hresult(factory->CreateForMonitor(monitor, guid_of<GraphicsCaptureItem>(), put_abi(item)));
        auto pool = Direct3D11CaptureFramePool::CreateFreeThreaded(captureDevice,
            winrt::Windows::Graphics::DirectX::DirectXPixelFormat::B8G8R8A8UIntNormalized, 1, item.Size());
        auto session = pool.CreateCaptureSession(item);
        if (observing) {
            MONITORINFO info{sizeof(MONITORINFO)};
            if (!GetMonitorInfoW(monitor, &info)) check_hresult(HRESULT_FROM_WIN32(GetLastError()));
            RECT roi{origin.x-info.rcMonitor.left, origin.y-info.rcMonitor.top,
                origin.x-info.rcMonitor.left+std::stoi(argv[4]), origin.y-info.rcMonitor.top+std::stoi(argv[5])};
            return observeRegion(device.get(),pool,session,roi,std::stoi(argv[6]));
        }
        if (argc == 2 && (std::string_view(argv[1]) == "--capture-test" ||
                          std::string_view(argv[1]) == "--control")) {
            session.Close(); pool.Close();
            return captureTest(std::string_view(argv[1]) == "--control", device.get(), captureDevice, item);
        }
        // Microsoft Learn IDisplayGraphicsCaptureSession, checked 2026-11-01:
        // Guid(3146905115,8586,22653,133,128,39,1,167,76,5,37).
        constexpr GUID displaySessionId{3146905115u, 8586, 22653,
            {133, 128, 39, 1, 167, 76, 5, 37}};
        com_ptr<IUnknown> extension;
        auto result = reinterpret_cast<IUnknown*>(get_abi(session))->QueryInterface(
            displaySessionId, extension.put_void());
        std::cout << "{\"status\":\""
            << (SUCCEEDED(result) ? "interface_available" :
                result == E_NOINTERFACE ? "interface_unavailable" : "query_failed")
            << "\",\"query_hresult\":\"0x" << std::hex << std::setfill('0')
            << std::setw(8) << static_cast<unsigned long>(result)
            << "\",\"capture_started\":false,\"affinity_changed\":false}\n";
        extension = nullptr;
        session.Close();
        pool.Close();
        return SUCCEEDED(result) || result == E_NOINTERFACE ? 0 : 1;
    } catch (hresult_error const& error) {
        std::cerr << "WGC capability probe failed: 0x" << std::hex
            << static_cast<unsigned long>(error.code().value) << " "
            << to_string(error.message()) << '\n';
        return 1;
    }
}
