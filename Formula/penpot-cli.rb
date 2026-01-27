class PenpotCli < Formula
  desc "Penpot CLI"
  homepage "https://github.com/radjathaher/penpot-cli"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/radjathaher/penpot-cli/releases/download/v0.1.0/penpot-cli-0.1.0-darwin-aarch64.tar.gz"
      sha256 "d66a15f73fe84716bcf2d46650aef926be6631ecf26fd91d146d8688e121c630"
    else
      odie "penpot-cli is only packaged for macOS arm64"
    end
  end

  def install
    bin.install "penpot"
  end
end
