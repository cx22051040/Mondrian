precision mediump float;

uniform vec2 u_resolution;  // 屏幕分辨率
uniform float u_time;       // 时间变量
uniform vec4 u_windowRect;  // 窗口区域 (x, y, width, height)
uniform float u_cornerRadius; // 圆角半径
uniform float u_borderWidth;   // 边框宽度
uniform vec3 u_color1;      // 渐变颜色1
uniform vec3 u_color2;      // 渐变颜色2

// 优化的圆角矩形SDF函数
float roundedBoxSDF(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

void main() {
    // 计算像素在窗口中的位置
    vec2 st = gl_FragCoord.xy - u_windowRect.xy;
    
    // 窗口中心点
    vec2 center = u_windowRect.zw * 0.5;
    
    // 相对于窗口中心的坐标
    vec2 p = st - center;
    
    // 计算符号距离
    float dist = roundedBoxSDF(p, center, u_cornerRadius);
    
    // 计算边框区域
    float border = abs(dist) - u_borderWidth;
    
    // 创建流动渐变效果
    float flow = fract(u_time * 0.5 + st.x * 0.02 + st.y * 0.03);
    vec3 borderColor = mix(u_color1, u_color2, flow);
    
    // 计算边框透明度 (使用平滑过渡)
    float alpha = smoothstep(0.0, 1.0, 1.0 - abs(border));
    
    // 仅绘制边框区域
    if (abs(dist) < u_borderWidth * 2.0) {
        // 在边框区域内应用颜色和透明度
        gl_FragColor = vec4(borderColor, alpha);
    } else {
        // 非边框区域完全透明
        discard;
    }
}