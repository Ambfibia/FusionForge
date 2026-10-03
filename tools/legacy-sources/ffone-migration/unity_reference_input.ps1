param(
    [Parameter(Mandatory = $true)]
    [string]$Text,

    [string]$WindowTitle = "FusionFall Retrobution Reference",

    [int]$ChatX = 200,

    [int]$ChatY = 663,

    [int]$CharacterDelayMilliseconds = 4,

    [switch]$DismissWarpDialog,

    [int]$DismissX = 940,

    [int]$DismissY = 385,

    [switch]$PressEscapeFirst,

    [switch]$NoSubmit
)

$ErrorActionPreference = "Stop"

if (-not ("UnityReference.NativeInput" -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

namespace UnityReference
{
    public static class NativeInput
    {
        public const uint INPUT_MOUSE = 0;
        public const uint INPUT_KEYBOARD = 1;
        public const uint KEYEVENTF_KEYUP = 0x0002;
        public const uint KEYEVENTF_UNICODE = 0x0004;
        public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
        public const uint MOUSEEVENTF_LEFTUP = 0x0004;
        public const ushort VK_MENU = 0x12;
        public const ushort VK_CONTROL = 0x11;
        public const ushort VK_RETURN = 0x0D;
        public const ushort VK_A = 0x41;
        public const ushort VK_BACK = 0x08;
        public const ushort VK_ESCAPE = 0x1B;

        [StructLayout(LayoutKind.Sequential)]
        public struct POINT
        {
            public int X;
            public int Y;
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct MOUSEINPUT
        {
            public int dx;
            public int dy;
            public uint mouseData;
            public uint dwFlags;
            public uint time;
            public UIntPtr dwExtraInfo;
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct KEYBDINPUT
        {
            public ushort wVk;
            public ushort wScan;
            public uint dwFlags;
            public uint time;
            public UIntPtr dwExtraInfo;
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct HARDWAREINPUT
        {
            public uint uMsg;
            public ushort wParamL;
            public ushort wParamH;
        }

        [StructLayout(LayoutKind.Explicit)]
        public struct INPUTUNION
        {
            [FieldOffset(0)] public MOUSEINPUT mi;
            [FieldOffset(0)] public KEYBDINPUT ki;
            [FieldOffset(0)] public HARDWAREINPUT hi;
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct INPUT
        {
            public uint type;
            public INPUTUNION data;
        }

        [DllImport("user32.dll", SetLastError = true)]
        public static extern uint SendInput(uint count, INPUT[] inputs, int size);

        [DllImport("user32.dll")]
        public static extern bool SetForegroundWindow(IntPtr window);

        [DllImport("user32.dll")]
        public static extern bool BringWindowToTop(IntPtr window);

        [DllImport("user32.dll")]
        public static extern bool ShowWindow(IntPtr window, int command);

        [DllImport("user32.dll")]
        public static extern bool ClientToScreen(IntPtr window, ref POINT point);

        [DllImport("user32.dll")]
        public static extern bool SetCursorPos(int x, int y);

        public static void Key(ushort virtualKey, bool keyUp)
        {
            INPUT input = new INPUT();
            input.type = INPUT_KEYBOARD;
            input.data.ki.wVk = virtualKey;
            input.data.ki.dwFlags = keyUp ? KEYEVENTF_KEYUP : 0;
            Send(new[] { input });
        }

        public static void Unicode(char character, bool keyUp)
        {
            INPUT input = new INPUT();
            input.type = INPUT_KEYBOARD;
            input.data.ki.wScan = character;
            input.data.ki.dwFlags = KEYEVENTF_UNICODE | (keyUp ? KEYEVENTF_KEYUP : 0);
            Send(new[] { input });
        }

        public static void Click()
        {
            INPUT down = new INPUT();
            down.type = INPUT_MOUSE;
            down.data.mi.dwFlags = MOUSEEVENTF_LEFTDOWN;
            INPUT up = new INPUT();
            up.type = INPUT_MOUSE;
            up.data.mi.dwFlags = MOUSEEVENTF_LEFTUP;
            Send(new[] { down, up });
        }

        private static void Send(INPUT[] inputs)
        {
            uint sent = SendInput((uint)inputs.Length, inputs, Marshal.SizeOf(typeof(INPUT)));
            if (sent != inputs.Length)
                throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        }
    }
}
'@
}

$process = Get-Process ffrunner -ErrorAction Stop |
    Where-Object { $_.MainWindowTitle -eq $WindowTitle } |
    Select-Object -First 1

if (-not $process) {
    throw "No ffrunner window with title '$WindowTitle' was found."
}

$window = [IntPtr]$process.MainWindowHandle
[void][UnityReference.NativeInput]::ShowWindow($window, 9)

# Tapping Alt allows a background automation process to transfer foreground focus.
[UnityReference.NativeInput]::Key([UnityReference.NativeInput]::VK_MENU, $false)
[UnityReference.NativeInput]::Key([UnityReference.NativeInput]::VK_MENU, $true)
[void][UnityReference.NativeInput]::BringWindowToTop($window)
[void][UnityReference.NativeInput]::SetForegroundWindow($window)
Start-Sleep -Milliseconds 150

if ($PressEscapeFirst) {
    [UnityReference.NativeInput]::Key([UnityReference.NativeInput]::VK_ESCAPE, $false)
    [UnityReference.NativeInput]::Key([UnityReference.NativeInput]::VK_ESCAPE, $true)
    Start-Sleep -Milliseconds 250
}

if ($DismissWarpDialog) {
    $dismissPoint = [UnityReference.NativeInput+POINT]::new()
    $dismissPoint.X = $DismissX
    $dismissPoint.Y = $DismissY
    if (-not [UnityReference.NativeInput]::ClientToScreen($window, [ref]$dismissPoint)) {
        throw "ClientToScreen failed for the dismiss point."
    }
    [void][UnityReference.NativeInput]::SetCursorPos($dismissPoint.X, $dismissPoint.Y)
    [UnityReference.NativeInput]::Click()
    Start-Sleep -Milliseconds 400
}

$point = [UnityReference.NativeInput+POINT]::new()
$point.X = $ChatX
$point.Y = $ChatY
if (-not [UnityReference.NativeInput]::ClientToScreen($window, [ref]$point)) {
    throw "ClientToScreen failed."
}

[void][UnityReference.NativeInput]::SetCursorPos($point.X, $point.Y)
[UnityReference.NativeInput]::Click()
Start-Sleep -Milliseconds 100

# Clear any text left in the chat edit box before entering the command.
[UnityReference.NativeInput]::Key([UnityReference.NativeInput]::VK_CONTROL, $false)
[UnityReference.NativeInput]::Key([UnityReference.NativeInput]::VK_A, $false)
[UnityReference.NativeInput]::Key([UnityReference.NativeInput]::VK_A, $true)
[UnityReference.NativeInput]::Key([UnityReference.NativeInput]::VK_CONTROL, $true)
[UnityReference.NativeInput]::Key([UnityReference.NativeInput]::VK_BACK, $false)
[UnityReference.NativeInput]::Key([UnityReference.NativeInput]::VK_BACK, $true)

foreach ($character in $Text.ToCharArray()) {
    [UnityReference.NativeInput]::Unicode($character, $false)
    [UnityReference.NativeInput]::Unicode($character, $true)
    if ($CharacterDelayMilliseconds -gt 0) {
        Start-Sleep -Milliseconds $CharacterDelayMilliseconds
    }
}

if (-not $NoSubmit) {
    [UnityReference.NativeInput]::Key([UnityReference.NativeInput]::VK_RETURN, $false)
    [UnityReference.NativeInput]::Key([UnityReference.NativeInput]::VK_RETURN, $true)
}

[PSCustomObject]@{
    ProcessId = $process.Id
    WindowHandle = $process.MainWindowHandle
    TextLength = $Text.Length
    Submitted = -not $NoSubmit
}
