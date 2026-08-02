cask "plugout" do
  version "0.4.4"
  sha256 "0a724f4734a58b0df13e5cb9872044d91e5aa97280e9976e957e1a887e655b32"

  url "https://github.com/ngriaznov/plugout/releases/download/v#{version}/plugout_#{version}_universal.dmg"
  name "plugout"
  desc "Audio plugin uninstaller"
  homepage "https://github.com/ngriaznov/plugout"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on macos: :big_sur

  app "plugout.app"

  zap trash: [
    "~/Library/Application Support/com.plugout.app",
    "~/Library/Caches/com.plugout.app",
    "~/Library/Preferences/com.plugout.app.plist",
    "~/Library/Saved Application State/com.plugout.app.savedState",
    "~/Library/WebKit/com.plugout.app",
  ]

  caveats <<~EOS
    plugout is not code-signed. If macOS refuses to open it, either install
    with --no-quarantine or run:
      xattr -cr /Applications/plugout.app
  EOS
end
