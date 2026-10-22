# Installing Network Mapper

This guide is for anyone running Network Mapper on their own PC. You don't need any
programming tools.

Network Mapper is still a prototype. It scans the network you choose for real: it pings each
address and checks common ports on the devices that answer. Only scan networks you own or
have permission to scan.

## What you need

| | |
| --- | --- |
| Windows | Windows 11, 64-bit |
| Graphics card | An NVIDIA graphics card with driver version 580 or newer |
| Disk space | About 700 MB for the app, plus 2.5 GB if you install the AI |
| Internet | Only to install the AI. Nothing else is downloaded, and nothing about your network is sent anywhere. |

There's nothing else to install: the files the app needs come in its folder.

### Checking your graphics driver

Open the **NVIDIA app** (or NVIDIA Control Panel → Help → System Information) and look at
the driver version. If it's older than 580, update it from the NVIDIA app or from
[nvidia.com/drivers](https://www.nvidia.com/drivers).

If your PC has no NVIDIA card, this copy of the app won't start. Ask for the CPU-only version
instead, which works on any PC but runs the AI more slowly.

## Installing

1. Copy the `network-mapper` folder anywhere you like, for example `Documents` or a USB stick.
   Keep everything in it together.
2. Open the folder and double-click `network-mapper.exe`.

The app has no installer and changes nothing on your PC outside its own data folder. A black
console window opens alongside the app; leave it open, because closing it closes the app.

The first time you scan, Windows may ask whether to allow Network Mapper on your network.
Allow it on **private** networks so scans can find devices and their names.

### If it doesn't start

| What you see | What to do |
| --- | --- |
| A message that a `.dll` file is missing | A file was left out when the folder was copied. Copy the whole folder again. |
| Nothing happens, or an error about the graphics card | Check the [graphics driver](#checking-your-graphics-driver). Without an NVIDIA card, use the CPU-only version. |
| Device icons are blank | Make sure the `icons` folder is still next to `network-mapper.exe`. |

## The AI (optional)

Everything works without the AI. It suggests names and types for the devices a scan finds.

The first time you choose **Import with AI**, the app offers to download AI support. Choose
**Download** and carry on using the app: the status bar at the bottom shows how far it has got,
and a message tells you when it's ready. **Cancel** beside the progress stops the download and
removes what was partly downloaded. You can also install it from **Settings → AI settings →
Install AI**.

The download is 2.5 GB and is checked before it's used. You only need the internet for this
download.

## Your files

- **Maps** are saved wherever you choose, as `.rhopper.json` files.
- **The AI and your AI settings** are kept in `%APPDATA%\Network Mapper\`. To open it, paste
  that path into the File Explorer address bar.

## Removing the app

1. Delete the `network-mapper` folder.
2. To also remove the AI and its settings, delete `%APPDATA%\Network Mapper\`.

Your saved maps stay where you saved them.
