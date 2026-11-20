# Loaded before launching playback, never compiled during a measured interval.
if (-not ('NidaPacingProcessCycles20261120' -as [type])) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class NidaPacingProcessCycles20261120 {
    [DllImport("kernel32.dll", SetLastError=true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool QueryProcessCycleTime(IntPtr process, out ulong cycles);
}
'@
}
function Get-PacingProcessCycles([System.Diagnostics.Process]$Process) {
    [UInt64]$cycles = 0
    if (-not [NidaPacingProcessCycles20261120]::QueryProcessCycleTime($Process.Handle, [ref]$cycles)) {
        throw 'Cannot sample diagnostic process CPU cycles'
    }
    return $cycles
}
# Warm the interop call before the diagnostic app starts.
$null = Get-PacingProcessCycles (Get-Process -Id $PID)
