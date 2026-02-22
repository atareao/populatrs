registry := "docker.io"
user     := "atareao"
name     := `basename ${PWD}`
version  := `vampus show`

list:
    @just --list

version:
    @vampus upgrade --patch

upgrade:
    #!/bin/fish
    vampus upgrade --patch
    set VERSION $(vampus show)
    git commit -am "Upgrade to version $VERSION"
    git tag -a "$VERSION" -m "Version $VERSION"
    # clean old docker images
    podman image list  | grep {{name}} | sort -r | tail -n +5 | awk '{print $2}' | while read id; echo $id; docker rmi $id; end
    just build push

build:
    @podman build \
        --tag {{registry}}/{{user}}/{{name}}:{{version}} \
        --tag {{registry}}/{{user}}/{{name}}:latest .

push:
    @podman push {{registry}}/{{user}}/{{name}}:{{version}}
    @podman push {{registry}}/{{user}}/{{name}}:latest
