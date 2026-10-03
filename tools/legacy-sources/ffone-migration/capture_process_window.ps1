param(
    [Parameter(Mandatory = $true)]
    [int]$TargetProcessId,

    [Parameter(Mandatory = $true)]
    [string]$OutputPath,

    [int]$TimeoutSeconds = 20
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

if (-not ('FFOneWindowCaptureNative' -as [type])) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class FFOneWindowCaptureNative
{
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct POINT
    {
        public int X;
        public int Y;
    }

    private delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr window);

    [DllImport("user32.dll")]
    public static extern bool GetClientRect(IntPtr window, out RECT rect);

    [DllImport("user32.dll")]
    public static extern bool ClientToScreen(IntPtr window, ref POINT point);

    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr window);

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr window, int command);

    public static IntPtr FindVisibleWindowForProcess(int wantedProcessId)
    {
        IntPtr found = IntPtr.Zero;
        EnumWindows(delegate(IntPtr window, IntPtr parameter)
        {
            uint processId;
            GetWindowThreadProcessId(window, out processId);
            if (processId != (uint)wantedProcessId || !IsWindowVisible(window))
                return true;

            RECT rect;
            if (!GetClientRect(window, out rect) || rect.Right <= rect.Left || rect.Bottom <= rect.Top)
                return true;

            found = window;
            return false;
        }, IntPtr.Zero);
        return found;
    }
}
'@
}

$deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
$window = [IntPtr]::Zero
while ([DateTime]::UtcNow -lt $deadline) {
    $process = Get-Process -Id $TargetProcessId -ErrorAction Stop
    $window = [FFOneWindowCaptureNative]::FindVisibleWindowForProcess($process.Id)
    if ($window -ne [IntPtr]::Zero) {
        break
    }
    Start-Sleep -Milliseconds 200
}

if ($window -eq [IntPtr]::Zero) {
    throw "No visible top-level window found for process $TargetProcessId."
}

[void][FFOneWindowCaptureNative]::ShowWindow($window, 9)
[void][FFOneWindowCaptureNative]::SetForegroundWindow($window)
Start-Sleep -Milliseconds 400

$client = New-Object FFOneWindowCaptureNative+RECT
if (-not [FFOneWindowCaptureNative]::GetClientRect($window, [ref]$client)) {
    throw "GetClientRect failed for process $TargetProcessId."
}
$origin = New-Object FFOneWindowCaptureNative+POINT
if (-not [FFOneWindowCaptureNative]::ClientToScreen($window, [ref]$origin)) {
    throw "ClientToScreen failed for process $TargetProcessId."
}
$width = $client.Right - $client.Left
$height = $client.Bottom - $client.Top
if ($width -le 0 -or $height -le 0) {
    throw "Invalid client size ${width}x${height} for process $TargetProcessId."
}

$resolvedOutput = [System.IO.Path]::GetFullPath($OutputPath)
$bitmap = New-Object System.Drawing.Bitmap($width, $height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
try {
    $graphics.CopyFromScreen(
        $origin.X,
        $origin.Y,
        0,
        0,
        (New-Object System.Drawing.Size($width, $height))
    )
    $bitmap.Save($resolvedOutput, [System.Drawing.Imaging.ImageFormat]::Png)
}
finally {
    $graphics.Dispose()
    $bitmap.Dispose()
}

[ordered]@{
    processId = $TargetProcessId
    window = $window.ToInt64()
    width = $width
    height = $height
    output = $resolvedOutput
} | ConvertTo-Json
