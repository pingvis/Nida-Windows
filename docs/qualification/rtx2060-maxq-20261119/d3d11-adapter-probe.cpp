#include <windows.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>
#include <iostream>
extern "C" {
#include <libavutil/hwcontext.h>
#include <libavutil/hwcontext_d3d11va.h>
}
using Microsoft::WRL::ComPtr;
int main(){
 AVBufferRef* reference=nullptr;
 int result=av_hwdevice_ctx_create(&reference,AV_HWDEVICE_TYPE_D3D11VA,nullptr,nullptr,0);
 std::cout<<"av_hwdevice_ctx_create="<<result<<"\n";
 if(result<0||!reference)return 1;
 auto* context=reinterpret_cast<AVHWDeviceContext*>(reference->data);
 auto* native=reinterpret_cast<AVD3D11VADeviceContext*>(context->hwctx);
 ComPtr<IDXGIDevice> device;
 HRESULT hr=native->device->QueryInterface(IID_PPV_ARGS(&device));
 if(FAILED(hr)){av_buffer_unref(&reference);return 2;}
 ComPtr<IDXGIAdapter> adapter;
 hr=device->GetAdapter(&adapter);
 if(FAILED(hr)){av_buffer_unref(&reference);return 3;}
 DXGI_ADAPTER_DESC desc{};adapter->GetDesc(&desc);
 std::wcout<<L"production_default_D3D11VA_adapter="<<desc.Description<<L" vendor="<<std::hex<<desc.VendorId<<L" device="<<desc.DeviceId<<L" luid="<<desc.AdapterLuid.HighPart<<L":"<<desc.AdapterLuid.LowPart<<std::dec<<L" dedicated_bytes="<<desc.DedicatedVideoMemory<<L"\n";
 av_buffer_unref(&reference);
 return 0;
}
