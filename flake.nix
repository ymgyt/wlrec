{
  description = "A CLI window recorder for Wayland";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { nixpkgs, ... }:
    {
      devShells = nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ] (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          gst = pkgs.gst_all_1;
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              just
              pkg-config
            ];
            buildInputs = [ gst.gstreamer ];
            GST_PLUGIN_SYSTEM_PATH_1_0 = pkgs.lib.makeSearchPath "lib/gstreamer-1.0" [
              gst.gstreamer
              gst.gst-plugins-base
              gst.gst-plugins-rs
              pkgs.pipewire
            ];
          };
        }
      );
    };
}
