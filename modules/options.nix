{ lib, package }:
{
  programs.quickpick = {
    enable = lib.mkEnableOption "the Quickpick cherry-pick tool";

    package = lib.mkOption {
      type = lib.types.package;
      default = package;
      description = "Quickpick package to install.";
    };
  };
}
