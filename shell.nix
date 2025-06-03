with import <nixpkgs> { };
stdenv.mkDerivation rec {
  name = "env";
  env = buildEnv {
    name = name;
    paths = buildInputs;
  };
  buildInputs = [
    soapysdr-with-plugins
    pkg-config
    llvm
    #cargo
    fontconfig.all
    pkg-config
    cairo.all
    gdk-pixbuf.all
    pango.all
    gtk4.all
    xorg.libX11.all
    xorg.libXcursor.all
    xorg.libXrandr.all
    xorg.libXi.all
    libGL.all
    libxkbcommon.dev
    libxkbcommon.out
    wayland.dev
    wayland.out
  ];

  # https://hoverbear.org/blog/rust-bindgen-in-nix/
  
  LIBCLANG_PATH = "${llvmPackages.libclang.lib}/lib";
  LD_LIBRARY_PATH="${xorg.libXcursor.out}/lib:${xorg.libXrandr.out}/lib:${xorg.libXi.out}/lib:${libGL.out}/lib:${wayland.out}/lib:${libxkbcommon.out}/lib";
  BINDGEN_EXTRA_CLANG_ARGS =
    "$(< ${stdenv.cc}/nix-support/libc-crt1-cflags) \n      $(< ${stdenv.cc}/nix-support/libc-cflags) \n      $(< ${stdenv.cc}/nix-support/cc-cflags) \n      $(< ${stdenv.cc}/nix-support/libcxx-cxxflags) \n      ${
            lib.optionalString stdenv.cc.isClang
            "-idirafter ${stdenv.cc.cc}/lib/clang/${
              lib.getVersion stdenv.cc.cc
            }/include"
          } \n      ${
            lib.optionalString stdenv.cc.isGNU
            "-isystem ${stdenv.cc.cc}/include/c++/${
              lib.getVersion stdenv.cc.cc
            } -isystem ${stdenv.cc.cc}/include/c++/${
              lib.getVersion stdenv.cc.cc
            }/${stdenv.hostPlatform.config} -idirafter ${stdenv.cc.cc}/lib/gcc/${stdenv.hostPlatform.config}/${
              lib.getVersion stdenv.cc.cc
            }/include"
          } \n    ";
}
