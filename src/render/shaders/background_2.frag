precision mediump float;
uniform vec2 u_resolution;
uniform float u_time;

const float SPEED_FACTOR = 0.3; // 速度控制参数

float opSmoothUnion(float d1, float d2, float k) {
    float h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}

float sdSphere(vec3 p, float s) {
    return length(p) - s;
}

float map(vec3 p) {
    float d = 2.0;
    for (int i = 0; i < 16; i++) {
        float fi = float(i);
        float time = u_time * SPEED_FACTOR * (fract(fi * 412.531 + 0.513) - 0.5) * 2.0;
        d = opSmoothUnion(
            sdSphere(
                p + sin(time + fi * vec3(52.5126, 64.62744, 632.25)) * vec3(2.0, 2.0, 0.8),
                mix(0.5, 1.0, fract(fi * 412.531 + 0.5124))
            ),
            d,
            0.4
        );
    }
    return d;
}

vec3 calcNormal(vec3 p) {
    const float h = 1e-5;
    const vec2 k = vec2(1, -1);
    return normalize(
        k.xyy * map(p + k.xyy * h) +
        k.yyx * map(p + k.yyx * h) +
        k.yxy * map(p + k.yxy * h) +
        k.xxx * map(p + k.xxx * h)
    );
}

void main() {
    // 坐标系转换
    vec2 uv = (2.0 * gl_FragCoord.xy - u_resolution.xy) / min(u_resolution.y, u_resolution.x);
    
    // 相机设置
    vec3 rayOri = vec3(0.0, 0.0, 3.0);
    vec3 rayDir = normalize(vec3(uv * 1.5, -1.0));
    
    float depth = 0.0;
    vec3 p;
    
    // 光线步进
    for (int i = 0; i < 64; i++) {
        p = rayOri + rayDir * depth;
        float dist = map(p);
        depth += dist * 0.8;
        if (dist < 1e-3 || depth > 15.0) {
            break;
        }
    }
    
    // 计算法线
    vec3 n = calcNormal(p);
    
    // 原始球体颜色计算
    float b = max(0.0, dot(n, vec3(0.577))); // 基础光照强度
    vec3 sphereCol = (0.5 + 0.5 * cos((b + u_time * SPEED_FACTOR * 3.0) + uv.xyx * 2.0 + vec3(0, 2, 4))) * 
                    (0.85 + b * 0.35); // 原始的颜色公式
    
    // 应用深度衰减
    sphereCol *= exp(-depth * 0.15);
    
    // ===================== 基于球体颜色的渐进式渐变背景 =====================
    // 计算从左下(0,0)到右上(1,1)的渐变因子
    vec2 screenUV = gl_FragCoord.xy / u_resolution;
    float gradientFactor = screenUV.x * 0.6 + screenUV.y * 0.4; // 对角线方向
    
    // 使用球体颜色生成背景颜色变体
    vec3 baseColor = sphereCol;
    
    // 创建左下角颜色（深色变体）
    vec3 bottomLeftColor = vec3(
        baseColor.r * 0.4 + 0.1 * sin(u_time * 0.3),
        baseColor.g * 0.5 + 0.1 * sin(u_time * 0.4 + 1.0),
        baseColor.b * 0.6 + 0.1 * sin(u_time * 0.5 + 2.0)
    );
    
    // 创建右上角颜色（亮色变体）
    vec3 topRightColor = vec3(
        baseColor.r * 1.2 + 0.1 * cos(u_time * 0.35),
        baseColor.g * 1.3 + 0.1 * cos(u_time * 0.45 + 1.0),
        baseColor.b * 1.4 + 0.1 * cos(u_time * 0.55 + 2.0)
    );
    
    // 应用对角线渐变
    vec3 bg = mix(bottomLeftColor, topRightColor, gradientFactor);
    
    // 添加与球体协调的色调
    bg = mix(bg, sphereCol * 0.4, 0.3); // 混合30%的球体颜色
    
    // 添加微妙的渐变纹理
    float pattern = sin(screenUV.x * 15.0 + u_time * 0.5) * 
                   cos(screenUV.y * 12.0 + u_time * 0.7) * 0.05;
    bg += pattern;
    // =================================================================
    
    // 添加雾效混合球体和背景
    float fog = smoothstep(3.0, 10.0, depth);
    vec3 finalCol = mix(sphereCol, bg, fog);
    
    // 增强颜色协调：将球体色调融入背景
    finalCol = mix(finalCol, sphereCol * 0.3 + bg * 0.7, 0.4);
    
    // 添加边缘光效果增强统一感
    float edge = 1.0 - smoothstep(0.0, 0.02, abs(map(p)));
    finalCol += sphereCol * edge * 0.4;
    
    gl_FragColor = vec4(finalCol, 1.0);
}