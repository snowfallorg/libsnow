#!@bash@/bin/sh
@desktop-file-utils@/bin/update-desktop-database -q "$HOME/.local/share/applications"

mkdir -p "$HOME/.local/share/icons/hicolor/"
cp -f @hicolor-icon-theme@/share/icons/hicolor/index.theme "$HOME/.local/share/icons/hicolor/"
for dir in "$HOME"/.local/share/icons/*; do
    if test -f "$dir/index.theme"; then
        if ! @gtk3@/bin/gtk-update-icon-cache --quiet "$dir"; then
            echo "Failed to run gtk-update-icon-cache for $dir"
            exit 1
        fi
    fi
done

exec @shared-mime-info@/bin/update-mime-database "$HOME/.local/share/mime"
