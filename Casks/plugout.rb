cask "plugout" do
  version "0.4.3"
  sha256 "510bd26aae7dcebb9f1ef73081ac41dd95e8479ae16c8b403b7bf960b807a24c"

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
