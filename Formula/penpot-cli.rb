class PenpotCli < Formula
  desc "Penpot CLI"
  homepage "https://github.com/radjathaher/penpot-cli"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/radjathaher/penpot-cli/releases/download/v0.1.0/penpot-cli-0.1.0-darwin-aarch64.tar.gz"
      sha256 "04d446e254f14b984dd03534f4913d73c024f128fac3c772f302ca8f2f2b3b29"
    else
      odie "penpot-cli is only packaged for macOS arm64"
    end
  end

  def install
    bin.install "penpot"
  end
end
