# This script takes care of building your crate and packaging it for release

set -ex

. $(dirname $0)/utils.sh

# Package your artifacts in a .deb file
# NOTE right now you can only package binaries using the `dobin` command. Simply call
# `dobin [file..]` to include one or more binaries in your .deb package. I'll add more commands to
# install other things like manpages (`doman`) as the needs arise.
# XXX This .deb packaging is minimal -- just to make your app installable via `dpkg` -- and doesn't
# fully conform to Debian packaging guideliens (`lintian` raises a few warnings/errors)
mk_deb() {
    dobin t_rex
}

mk_package() {
    if [ $TRAVIS_OS_NAME = linux ]; then
        if [ ! -z $MAKE_DEB ]; then
            # Build trusty deb package
            sudo apt-get -qq update
            sudo apt-get install -y fakeroot

            dtd=$(mktemp -d)
            mkdir -p $dtd/debian/usr/bin

            mk_deb

            mkdir -p $dtd/debian/DEBIAN
            cat >$dtd/debian/DEBIAN/control <<EOF
Package: $CRATE_NAME
Version: ${TRAVIS_TAG#v}
Architecture: $(architecture $TARGET)
Maintainer: $DEB_MAINTAINER
Description: $DEB_DESCRIPTION
Depends: openssl, libgdal20
EOF

            fakeroot dpkg-deb --build $dtd/debian
            mv $dtd/debian.deb $src/$CRATE_NAME-$TRAVIS_TAG-$TARGET.deb
            rm -r $dtd

            # Build buster deb package
            docker build -t build-t-rex-buster -f $src/ci/Dockerfile-buster $src
            docker run --rm --user $(id -u):$(id -g) -v $src:/dest build-t-rex-buster cp target/debian/t-rex_${TRAVIS_TAG#v}_amd64.deb /dest/t-rex_${TRAVIS_TAG#v}_amd64_buster.deb

            # Build Docker container
            cd $src/packaging/docker
            cp $src/$CRATE_NAME-$TRAVIS_TAG-$TARGET.deb .
            # travis uid/gid is 2000/2000, but we build with default uid 1000
            docker build -t sourcepole/t-rex -f Dockerfile .
            docker run sourcepole/t-rex --version

            docker login -u="$DOCKER_USERNAME" -p="$DOCKER_PASSWORD"
            docker push sourcepole/t-rex
            docker tag sourcepole/t-rex sourcepole/t-rex:${TRAVIS_TAG#v}
            docker push sourcepole/t-rex:${TRAVIS_TAG#v}
        fi
   fi
}

main() {
    local src=$(pwd) \
          stage=

    case $TRAVIS_OS_NAME in
        linux)
            stage=$(mktemp -d)
            ;;
        osx)
            stage=$(mktemp -d -t tmp)
            ;;
    esac

    test -f Cargo.lock || cargo generate-lockfile

    cargo rustc --bin t_rex --target $TARGET --release -- -C lto

    cp target/$TARGET/release/t_rex $stage/

    cd $stage
    tar czf $src/$CRATE_NAME-$TRAVIS_TAG-$TARGET.tar.gz *

    mk_package

    cd $src

    rm -rf $stage
}

main
