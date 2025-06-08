precision mediump float;

uniform vec2 u_resolution;
uniform vec3 border_color;
uniform float border_thickness;
uniform float u_time;
uniform float corner_radius;

varying vec2 v_coords;

void main() {
    vec2 coords = v_coords * u_resolution;

    // 圆角裁剪
    float r = corner_radius;
    vec2 size = u_resolution;

    // 计算每个角落中心点
    vec2 topLeft = vec2(r, r);
    vec2 topRight = vec2(size.x - r, r);
    vec2 bottomLeft = vec2(r, size.y - r);
    vec2 bottomRight = vec2(size.x - r, size.y - r);

    // // 标志位：是否在圆角外
    // bool in_corner = false;
    // if (coords.x < r && coords.y < r) {
    //     in_corner = distance(coords, topLeft) > r;
    // } else if (coords.x > size.x - r && coords.y < r) {
    //     in_corner = distance(coords, topRight) > r;
    // } else if (coords.x < r && coords.y > size.y - r) {
    //     in_corner = distance(coords, bottomLeft) > r;
    // } else if (coords.x > size.x - r && coords.y > size.y - r) {
    //     in_corner = distance(coords, bottomRight) > r;
    // }

    // if (in_corner) discard;

    // 边框区域
    float inside_left = step(border_thickness, coords.x);
    float inside_right = step(coords.x, size.x - border_thickness);
    float inside_top = step(border_thickness, coords.y);
    float inside_bottom = step(coords.y, size.y - border_thickness);

    float inside_mask = inside_left * inside_right * inside_top * inside_bottom;
    if (inside_mask == 1.0) discard;

    float border_mask = 1.0 - inside_mask;

    // 呼吸动画
    float pulse = 0.3 + 0.7 * abs(sin(u_time * 1.5));
    float alpha = border_mask * pulse;

    // 渐变颜色
    vec3 gradient_color = mix(vec3(0.7333, 0.7922, 0.9882), vec3(0.2745, 0.1647, 0.9137), coords.y / size.y);
    vec3 final_color = mix(border_color, gradient_color, 0.5);

    gl_FragColor = vec4(final_color, alpha);
}
