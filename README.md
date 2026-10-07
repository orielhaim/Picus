# Picus

Sleep deprivation + boredom + a Raspberry Pi Pico collecting dust in a drawer + absolutely nothing better to do = whatever the hell this is

Basically I made a tiny Raspberry Pi Pico pretend to be a USB drive with almost 8 exabytes of storage

It doesn't actually store every combination of data in existence! By generating files mathematically based on the folders you navigate through. Somewhere in that ridiculous maze of folders is literally any file you could ever want

It's all written in Rust because apparently I wasn't suffering enough

## How to run this thing

**Step 1:** Seriously reconsider whether you have anything better to do with your life

**Step 2:** If you're still reading, well, welcome to the club. Here's how to waste your time:

You'll need a Raspberry Pi Pico, Rust, `picotool`, and Bun

1. Hold BOOTSEL while plugging in your Pico
2. Flash this masterpiece:

   ```bash
   cargo run --release
   ```

3. Generate the path to literally any file:

   ```bash
   bun f2p.js ./yourfile.txt
   ```

4. Open the Pico in Windows Explorer, follow the generated folder path, and find your file

Yes it actually works. No I don't know why I spent my time on this

Have fun wasting yours
