# AUR packaging

This directory holds the AUR `PKGBUILD` recipes for `constellation`.
They live in the source repo for review and version bumps, but the
actual package lives on AUR (a separate git remote).

| recipe                                  | tracks                                     | how                                |
| :-------------------------------------- | :----------------------------------------- | :--------------------------------- |
| `constellation-bin/PKGBUILD`            | latest tagged GitHub Release pre-built     | downloads the `*-linux-gnu.tar.xz` |
| `constellation-git/PKGBUILD` (planned)  | latest commit on `main`, builds from source | `cargo build --release`            |

## publishing a new version to AUR (first time)

```sh
# clone the AUR git repo (one time)
git clone ssh://aur@aur.archlinux.org/constellation-bin.git aur-constellation-bin

# copy the recipe across
cp packaging/aur/constellation-bin/PKGBUILD aur-constellation-bin/

cd aur-constellation-bin

# refresh checksums for the new release tarballs
updpkgsums

# regenerate .SRCINFO (AUR mandatory metadata file)
makepkg --printsrcinfo > .SRCINFO

# local sanity build (optional but recommended)
makepkg -si

# commit and push
git add PKGBUILD .SRCINFO
git commit -m "upgpkg: constellation-bin 0.1.0-1"
git push
```

## bumping the version

1. Update `pkgver=` and reset `pkgrel=1` in `packaging/aur/constellation-bin/PKGBUILD`.
2. Run the same six commands above from the AUR clone (steps 2-6).

## prerequisites

- An [AUR account](https://aur.archlinux.org/register) with an SSH key registered.
- A working `base-devel` + `pacman-contrib` (for `updpkgsums`) install.
