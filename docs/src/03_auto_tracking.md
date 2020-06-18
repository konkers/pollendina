# Auto Tracking

## Setup

Pollendina needs a usb2snes service for auto tracking. Two programs provide
this service:

- [usb2snes](https://github.com/RedGuyyyy/sd2snes/releases) (Windows only)
- [QUsb2snes](https://skarsnik.github.io/QUsb2snes/) (Windows, Mac, Linux)

QUsb2snes is probably a better choice. It's newer, has more features and
support more platforms.

The [SMZ3r multiworld setup instructions](https://skarsnik.github.io/QUsb2snes/)
cover getting these programs set up on both SD2SNES(FXPAK) and Emulator.

The following configurations have been tested:

|                            | Windows | Mac | Linux |
| -------------------------- | ------- | --- | ----- |
| SD2SNES(FXPAK)             | ✅      | ✅  | ✅    |
| Snes9x-rr-1.55-multitroid2 | ✅      | 🟡  | 🟡    |
| bsnes-mercury-balanced     | 🟡      | 🟡  | ✅    |

- ✅ Know working
- 🟡 Untested
- ❌ Know not working

## Usage

To use auto-tracking:

- Make sure you have the usb2snes service set up per the above instructions.
- Load an Free Enterprise rom.
- Click the `Start auto tracking` button.
  - **_NOTE!!! Pollendina will only connect to the first usb2snes devices it sees._**
- If all goes well you should see the `Idle` status change to `Connected`
