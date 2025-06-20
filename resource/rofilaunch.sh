#!/usr/bin/env sh

# Rofi 样式编号，对应 style_*.rasi
rofiStyle="1"

# 字体大小（整数）
rofiScale="10"

# 窗口宽度 / 边框设置
width=2
border=4

# rofi 配置目录（根据实际路径修改）
confDir="${HOME}/.config"

# ===== 🗂️ 自动选择主题文件 =====

roconf="${confDir}/rofi/styles/style_${rofiStyle}.rasi"

# fallback: 如果指定样式不存在，就选第一个可用样式
if [ ! -f "${roconf}" ]; then
    roconf="$(find "${confDir}/rofi/styles" -type f -name "style_*.rasi" | sort -t '_' -k 2 -n | head -1)"
fi

# ===== 🧭 参数解析（运行模式） =====

case "${1}" in
    d|--drun) r_mode="drun" ;;
    w|--window) r_mode="window" ;;
    f|--filebrowser) r_mode="filebrowser" ;;
    h|--help)
        echo -e "$(basename "${0}") [action]"
        echo "d :  drun mode"
        echo "w :  window mode"
        echo "f :  filebrowser mode"
        exit 0
        ;;
    *) r_mode="drun" ;;
esac

# ===== 🎨 动态样式注入 =====

wind_border=$(( border * 3 ))
[ "${border}" -eq 0 ] && elem_border=10 || elem_border=$(( border * 2 ))

r_override="window {border: ${width}px; border-radius: ${wind_border}px;} element {border-radius: ${elem_border}px;}"
r_scale="configuration {font: \"JetBrainsMono Nerd Font ${rofiScale}\";}"

# 获取当前 GNOME 图标主题（如果可用）
if command -v gsettings >/dev/null; then
    i_theme="$(gsettings get org.gnome.desktop.interface icon-theme | sed "s/'//g")"
    i_override="configuration {icon-theme: \"${i_theme}\";}"
else
    i_override=""
fi

# ===== 🚀 启动 rofi =====

rofi -show "${r_mode}" \
     -theme-str "${r_scale}" \
     -theme-str "${r_override}" \
     -theme-str "${i_override}" \
     -config "${roconf}"
