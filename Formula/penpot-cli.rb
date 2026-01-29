class PenpotCli < Formula
  desc "Penpot CLI"
  homepage "https://github.com/radjathaher/penpot-cli"
  version "0.2.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/radjathaher/penpot-cli/releases/download/v0.2.0/penpot-cli-0.2.0-darwin-aarch64.tar.gz"
      sha256 "bd4a78165be32f06d73e6e0098d1233110cb8c8bece4b23791985d8e570ccbf7"
    else
      odie "penpot-cli is only packaged for macOS arm64"
    end
  end

  def install
    bin.install "penpot"
  end
end
