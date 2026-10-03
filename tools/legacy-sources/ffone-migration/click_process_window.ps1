param(
    [Parameter(Mandatory = $true)]
    [int]$TargetProcessId,

    [Parameter(Mandatory = $true)]
    [int]$ClientX,

    [Parameter(Mandatory = $true)]
    [int]$ClientY,

    [int]$TimeoutSeconds = 20
)

$ErrorActionPreference = 'Stop'

if (-not ('FFOneWindowInputNative' -as [type])) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class FFOneWindowInputNative
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

    [DllImport("user32.dll")]
    private static extern bool SetCursorPos(int x, int y);

    [DllImport("user32.dll")]
    private static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extraInfo);

    private const uint MouseLeftDown = 0x0002;
    private const uint MouseLeftUp = 0x0004;

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

    public static void ClickClient(IntPtr window, int clientX, int clientY)
    {
        RECT rect;
        if (!GetClientRect(window, out rect))
            throw new InvalidOperationException("GetClientRect failed.");
        int width = rect.Right - rect.Left;
        int height = rect.Bottom - rect.Top;
        if (clientX < 0 || clientY < 0 || clientX >= width || clientY >= height)
            throw new ArgumentOutOfRangeException("client coordinate");

        POINT point = new POINT { X = clientX, Y = clientY };
        if (!ClientToScreen(window, ref point))
            throw new InvalidOperationException("ClientToScreen failed.");
        if (!SetCursorPos(point.X, point.Y))
            throw new InvalidOperationException("SetCursorPos failed.");
        mouse_event(MouseLeftDown, 0, 0, 0, UIntPtr.Zero);
        mouse_event(MouseLeftUp, 0, 0, 0, UIntPtr.Zero);
    }
}
'@
}

$deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
$window = [IntPtr]::Zero
while ([DateTime]::UtcNow -lt $deadline) {
    $process = Get-Process -Id $TargetProcessId -ErrorAction Stop
    $window = [FFOneWindowInputNative]::FindVisibleWindowForProcess($process.Id)
    if ($window -ne [IntPtr]::Zero) {
        break
    }
    Start-Sleep -Milliseconds 200
}

if ($window -eq [IntPtr]::Zero) {
    throw "No visible top-level window found for process $TargetProcessId."
}

[void][FFOneWindowInputNative]::ShowWindow($window, 9)
[void][FFOneWindowInputNative]::SetForegroundWindow($window)
Start-Sleep -Milliseconds 250
[FFOneWindowInputNative]::ClickClient($window, $ClientX, $ClientY)

[ordered]@{
    processId = $TargetProcessId
    window = $window.ToInt64()
    clientX = $ClientX
    clientY = $ClientY
} | ConvertTo-Json
