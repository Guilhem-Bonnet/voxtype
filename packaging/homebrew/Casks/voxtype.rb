cask "voxtype" do
  version "0.6.0-rc1"
  sha256 "ad5c4f2531ed50ed028ec7e85062abeb2e64c27e8d1becb84b4946b631ba7aeb"

  url "https://github.com/peteonrails/voxtype/releases/download/v#{version}/Voxtype-#{version}-macos-arm64.dmg"
  name "Voxtype"
  desc "Push-to-talk voice-to-text for macOS"
  homepage "https://voxtype.io"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :ventura"
  depends_on formula: "terminal-notifier"

  app "Voxtype.app"

  postflight do
    # Remove quarantine attribute (app is unsigned)
    system_command "/usr/bin/xattr", args: ["-cr", "/Applications/Voxtype.app"]

    # Clean up any stale state from previous installs
    system_command "/bin/rm", args: ["-rf", "/tmp/voxtype"]

    # Create config directory
    system_command "/bin/mkdir", args: ["-p", "#{ENV["HOME"]}/Library/Application Support/voxtype"]

    # Create logs directory
    system_command "/bin/mkdir", args: ["-p", "#{ENV["HOME"]}/Library/Logs/voxtype"]

    # Bundle terminal-notifier for notifications with custom icon
    system_command "/bin/cp", args: [
      "-R",
      "#{HOMEBREW_PREFIX}/opt/terminal-notifier/terminal-notifier.app",
      "/Applications/Voxtype.app/Contents/Resources/"
    ]

    # Create symlink for CLI access
    system_command "/bin/ln", args: ["-sf", "/Applications/Voxtype.app/Contents/MacOS/voxtype", "#{HOMEBREW_PREFIX}/bin/voxtype"]

    # Install LaunchAgent for auto-start
    launch_agents_dir = "#{ENV["HOME"]}/Library/LaunchAgents"
    system_command "/bin/mkdir", args: ["-p", launch_agents_dir]

    plist_path = "#{launch_agents_dir}/io.voxtype.daemon.plist"
    plist_content = <<~PLIST
      <?xml version="1.0" encoding="UTF-8"?>
      <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
      <plist version="1.0">
      <dict>
          <key>Label</key>
          <string>io.voxtype.daemon</string>
          <key>ProgramArguments</key>
          <array>
              <string>/Applications/Voxtype.app/Contents/MacOS/voxtype</string>
              <string>daemon</string>
          </array>
          <key>RunAtLoad</key>
          <true/>
          <key>KeepAlive</key>
          <true/>
          <key>StandardOutPath</key>
          <string>#{ENV["HOME"]}/Library/Logs/voxtype/stdout.log</string>
          <key>StandardErrorPath</key>
          <string>#{ENV["HOME"]}/Library/Logs/voxtype/stderr.log</string>
          <key>EnvironmentVariables</key>
          <dict>
              <key>PATH</key>
              <string>/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin</string>
          </dict>
          <key>ProcessType</key>
          <string>Interactive</string>
          <key>Nice</key>
          <integer>-10</integer>
      </dict>
      </plist>
    PLIST

    File.write(plist_path, plist_content)

    # Load the LaunchAgent
    system_command "/bin/launchctl", args: ["load", plist_path]
  end

  uninstall_postflight do
    # Unload and remove LaunchAgent
    plist_path = "#{ENV["HOME"]}/Library/LaunchAgents/io.voxtype.daemon.plist"
    system_command "/bin/launchctl", args: ["unload", plist_path] if File.exist?(plist_path)
    system_command "/bin/rm", args: ["-f", plist_path]

    # Remove CLI symlink
    system_command "/bin/rm", args: ["-f", "#{HOMEBREW_PREFIX}/bin/voxtype"]
  end

  uninstall quit: "io.voxtype.app"

  zap trash: [
    "~/Library/Application Support/voxtype",
    "~/Library/LaunchAgents/io.voxtype.daemon.plist",
    "~/Library/Logs/voxtype",
  ]

  caveats <<~EOS
    Voxtype is installed and will start automatically at login.

    First-time setup:

    1. If prompted "Voxtype was blocked", go to System Settings >
       Privacy & Security and click "Open Anyway"

    2. Download a speech model:
       voxtype setup --download --model parakeet-tdt-0.6b-v3-int8

    3. Grant Input Monitoring permission in System Settings >
       Privacy & Security > Input Monitoring (required for hotkey)

    Default hotkey: Right Option (hold to record, release to transcribe)

    For menu bar status icon: voxtype menubar
  EOS
end
