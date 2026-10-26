{ pkgs, inputs, ... }:

{
  # https://devenv.sh/packages/
  packages =
    with pkgs;
    [
      evcxr
      gitoxide
      jujutsu
    ]
    ++ [ inputs.openspec.packages.${pkgs.system}.default ];

  dotenv.enable = true;

  languages.nix.enable = true;

  languages.python = {
    enable = true;
    uv.enable = true;
  };

  languages.rust = {
    enable = true;
    channel = "stable";
    components = [
      "rustc"
      "cargo"
      "clippy"
      "rustfmt"
      "rust-analyzer"
    ];
  };

  # https://devenv.sh/git-hooks/
  # git-hooks.hooks.shellcheck.enable = true;

  # See full reference at https://devenv.sh/reference/options/
}
