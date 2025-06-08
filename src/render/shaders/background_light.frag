#ifdef GL_ES
precision mediump float;
#endif

uniform vec2 u_resolution;
uniform float u_time;
varying vec2 v_texcoord;

// 距离场函数 - 旋转后的Gyroid结构
float g(vec4 p, float s) {
  return abs(dot(sin(p *= s), cos(p.zxwy)) - 1.) / s;
}

void main() {
  vec2 C = gl_FragCoord.xy;
  // 翻转y轴以匹配Shadertoy坐标系
  C.y = u_resolution.y - C.y;
  
  float T = u_time;
  float i = 0.0, d, z = 0.0, s;
  vec4 o = vec4(0.0), q, p, U = vec4(2.0, 1.0, 0.0, 3.0);
  vec2 r = u_resolution.xy;
  
  // 修复的for循环语法 - 使用标准格式
  for (int j = 0; j < 78; j++) {
    i += 1.0;
    
    // 计算光线位置
    q = vec4(normalize(vec3(C - 0.5 * r, r.y)) * z, 0.2);
    q.z += T / 30.0;  // 随时间推进隧道
    
    // 创建水面反射效果
    s = q.y + 0.1;
    q.y = abs(s);
    
    // 准备距离场计算
    p = q;
    p.y -= 0.11;
    
    // 旋转隧道壁
    float angle = -2.0 * p.z;
    mat2 rot = mat2(
      cos(angle + 11.0 * U.z), sin(angle + 11.0 * U.z),
      -sin(angle + 33.0 * U.w), cos(angle + 33.0 * U.w)
    );
    p.xy *= rot;
    p.y -= 0.2;
    
    // 计算组合距离场
    d = abs(g(p, 8.0) - g(p, 24.0)) / 4.0;
    
    // 基础发光颜色
    vec4 base = 1.0 + cos(0.7 * U + 5.0 * q.z);
    
    // 累积发光效果
    float factor = (s > 0.0) ? 1.0 : 0.1;
    float denominator = (s > 0.0) ? d : d * d * d;
    o += factor * base.w * base / max(denominator, 5E-4);
    
    // 更新步进距离
    z += d + 5E-4;
  }
  
  // 添加隧道末端的脉动发光体
  float pulse = 1.4 + sin(T) * sin(1.7 * T) * sin(2.3 * T);
  o += pulse * 1000.0 * U / length(q.xy);
  
  // 应用色调映射并输出
  vec4 O = (o / 1e5) / (1.0 + abs(o / 1e5));
  gl_FragColor = O;
}