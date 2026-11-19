#include <windows.h>
#include <initguid.h>
#include <dxgi1_6.h>
#include <d3d12.h>
#include <d3d12video.h>
#include <wrl/client.h>
#include <iostream>
#include <vector>
using Microsoft::WRL::ComPtr;
int main() {
 ComPtr<IDXGIFactory6> factory;
 HRESULT hr=CreateDXGIFactory2(0,IID_PPV_ARGS(&factory));
 if(FAILED(hr)){std::cout<<"factory_hresult="<<std::hex<<hr<<"\n";return 1;}
 for(UINT index=0;;++index){
  ComPtr<IDXGIAdapter1> adapter;
  hr=factory->EnumAdapterByGpuPreference(index,DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,IID_PPV_ARGS(&adapter));
  if(hr==DXGI_ERROR_NOT_FOUND)break;
  if(FAILED(hr))return 2;
  DXGI_ADAPTER_DESC1 desc{};adapter->GetDesc1(&desc);
  std::wcout<<L"adapter="<<index<<L" name="<<desc.Description<<L" vendor="<<std::hex<<desc.VendorId<<L" device="<<desc.DeviceId<<L" luid="<<desc.AdapterLuid.HighPart<<L":"<<desc.AdapterLuid.LowPart<<std::dec<<L" dedicated_bytes="<<desc.DedicatedVideoMemory<<L" flags="<<desc.Flags<<L"\n";
  ComPtr<ID3D12Device> device;
  hr=D3D12CreateDevice(adapter.Get(),D3D_FEATURE_LEVEL_11_0,IID_PPV_ARGS(&device));
  std::cout<<"d3d12_hresult="<<std::hex<<hr<<std::dec<<"\n";
  if(FAILED(hr))continue;
  ComPtr<ID3D12VideoDevice> video;
  hr=device.As(&video);std::cout<<"video_device_hresult="<<std::hex<<hr<<std::dec<<"\n";
  if(FAILED(hr))continue;
  D3D12_FEATURE_DATA_VIDEO_DECODE_PROFILE_COUNT count{};
  hr=video->CheckFeatureSupport(D3D12_FEATURE_VIDEO_DECODE_PROFILE_COUNT,&count,sizeof(count));
  std::cout<<"profile_count_hresult="<<std::hex<<hr<<std::dec<<" count="<<count.ProfileCount<<"\n";
  if(FAILED(hr))continue;
  std::vector<GUID> profiles(count.ProfileCount);
  D3D12_FEATURE_DATA_VIDEO_DECODE_PROFILES list{};list.ProfileCount=count.ProfileCount;list.pProfiles=profiles.data();
  hr=video->CheckFeatureSupport(D3D12_FEATURE_VIDEO_DECODE_PROFILES,&list,sizeof(list));
  if(FAILED(hr)){std::cout<<"profiles_hresult="<<std::hex<<hr<<std::dec<<"\n";continue;}
  for(const auto &profile:profiles){wchar_t text[64]{};StringFromGUID2(profile,text,64);std::wcout<<L"profile="<<text;
   if(IsEqualGUID(profile,D3D12_VIDEO_DECODE_PROFILE_H264))std::wcout<<L" H264";
   if(IsEqualGUID(profile,D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN))std::wcout<<L" HEVC_MAIN";
   if(IsEqualGUID(profile,D3D12_VIDEO_DECODE_PROFILE_HEVC_MAIN10))std::wcout<<L" HEVC_MAIN10";
   if(IsEqualGUID(profile,D3D12_VIDEO_DECODE_PROFILE_AV1_PROFILE0))std::wcout<<L" AV1_PROFILE0";
   std::wcout<<L"\n";
  }
 }
 return 0;
}
