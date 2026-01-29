class PenpotCli < Formula
  desc "Penpot CLI"
  homepage "https://github.com/radjathaher/penpot-cli"
  version "0.2.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/radjathaher/penpot-cli/releases/download/v0.2.1/penpot-cli-0.2.1-darwin-aarch64.tar.gz"
      sha256 "86e782f5e2c1c032855dfebef7666ddc69b02455d9760e953e0c128bb80dfc75"
    else
      odie "penpot-cli is only packaged for macOS arm64"
    end
  end

  def install
    bin.install "penpot"
  end
end
