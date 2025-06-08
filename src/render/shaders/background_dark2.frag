#ifdef GL_ES
precision mediump float;
#endif

uniform vec2 u_resolution;
uniform float u_time;
uniform vec2 u_mouse;
varying vec2 v_texcoord;

#define ZERO_TRICK 0
//#define MANUAL_CAMERA  // 取消注释启用鼠标控制

// 全局变量
float localTime = 0.0;
float seed = 1.0;
float animStructure = 1.0;
float fade = 1.0;
float pulse;

// 噪声和工具函数保持不变
float v31(vec3 a) { return a.x + a.y * 37.0 + a.z * 521.0; }
float v21(vec2 a) { return a.x + a.y * 37.0; }
float Hash11(float a) { return fract(sin(a)*10403.9); }
float Hash21(vec2 uv) { return fract(sin(v21(uv))*104003.9); }
vec2 Hash22(vec2 uv) { return fract(cos(v21(uv))*vec2(10003.579, 37049.7)); }
float Hash1d(float u) { return fract(sin(u)*143.9); }
float Hash2d(vec2 uv) { return fract(sin(v21(uv))*104003.9); }
float Hash3d(vec3 uv) { return fract(sin(v31(uv))*110003.9); }
float mixP(float f0, float f1, float a) { return mix(f0, f1, a*a*(3.0-2.0*a)); }
const vec2 zeroOne = vec2(0.0, 1.0);
float noise2d(vec2 uv) {
    vec2 fr = fract(uv.xy);
    vec2 fl = floor(uv.xy);
    float h00 = Hash2d(fl);
    float h10 = Hash2d(fl + zeroOne.yx);
    float h01 = Hash2d(fl + zeroOne);
    float h11 = Hash2d(fl + zeroOne.yy);
    return mixP(mixP(h00, h10, fr.x), mixP(h01, h11, fr.x), fr.y);
}
float noise(vec3 uv) {
    vec3 fr = fract(uv.xyz);
    vec3 fl = floor(uv.xyz);
    float h000 = Hash3d(fl);
    float h100 = Hash3d(fl + zeroOne.yxx);
    float h010 = Hash3d(fl + zeroOne.xyx);
    float h110 = Hash3d(fl + zeroOne.yyx);
    float h001 = Hash3d(fl + zeroOne.xxy);
    float h101 = Hash3d(fl + zeroOne.yxy);
    float h011 = Hash3d(fl + zeroOne.xyy);
    float h111 = Hash3d(fl + zeroOne.yyy);
    return mixP(mixP(mixP(h000, h100, fr.x), mixP(h010, h110, fr.x), fr.y),
               mixP(mixP(h001, h101, fr.x), mixP(h011, h111, fr.x), fr.y), fr.z);
}

const float PI = 3.14159265;

vec3 saturate(vec3 a) { return clamp(a, 0.0, 1.0); }
vec2 saturate(vec2 a) { return clamp(a, 0.0, 1.0); }
float saturate(float a) { return clamp(a, 0.0, 1.0); }

vec3 RotateX(vec3 v, float rad) {
    float cos = cos(rad);
    float sin = sin(rad);
    return vec3(v.x, cos * v.y + sin * v.z, -sin * v.y + cos * v.z);
}
vec3 RotateY(vec3 v, float rad) {
    float cos = cos(rad);
    float sin = sin(rad);
    return vec3(cos * v.x - sin * v.z, v.y, sin * v.x + cos * v.z);
}
vec3 RotateZ(vec3 v, float rad) {
    float cos = cos(rad);
    float sin = sin(rad);
    return vec3(cos * v.x + sin * v.y, -sin * v.x + cos * v.y, v.z);
}

float DecayNoise3D(vec3 p) {
    float n = 0.0;
    float iter = 1.0;
    float pn = noise(p*0.1) * 1.5;
    pn += noise(p*0.2) * 0.75;
    pn += noise(p*0.4) * 0.375;
    pn += noise(p*0.8) * 0.1875;
    for (int i = ZERO_TRICK; i < 8; i++) {
        float wave = saturate(cos(p.y*0.3 + pn + localTime*0.1) - 0.995);
        wave *= noise(p * 0.15) * 1024.0;
        n += wave * (1.0 + sin(p.x + p.z)*0.5);
        p.xy += vec2(p.y, -p.x) * 0.68;
        p.xy *= 1.0 / sqrt(1.0 + 0.68*0.68);
        p.xz += vec2(p.z, -p.x) * 0.68;
        p.xz *= 1.0 / sqrt(1.0 + 0.68*0.68);
        iter *= 1.523;
    }
    return n * 0.8;
}

float repsDouble(float a) { return abs(a * 2.0 - 1.0); }
vec2 repsDouble(vec2 a) { return abs(a * 2.0 - 1.0); }

vec2 mapAbyssalSpiral(vec2 uv) {
    float len = length(uv);
    float at = atan(uv.x, uv.y) / PI;
    float dist = (fract(log(len*1.2)+at*0.7)-0.5) * 2.0;
    at = repsDouble(at);
    at = repsDouble(at);
    dist += sin(at*24.0 + localTime*0.5)*0.1;
    return vec2(abs(dist), abs(at));
}

vec3 mapVoidInvert(vec3 uv) {
    float len = length(uv);
    vec3 dir = normalize(uv);
    len = 1.2 / (len + 0.1);
    return dir * len;
}

float length8(vec2 v) { return pow(pow(abs(v.x), 8.0) + pow(abs(v.y), 8.0), 1.0/8.0); }

float sdBox(vec3 p, vec3 radius) {
    vec3 dist = abs(p) - radius;
    return min(max(dist.x, max(dist.y, dist.z)), 0.0) + length(max(dist, 0.0));
}

float sdTorusWraith(vec3 p, vec2 t, float offset) {
    float a = atan(p.x, p.z);
    float subs = 2.5;
    a = sin(a*subs + localTime*2.5 + offset*4.567);
    vec2 q = vec2(length(p.xz)-t.x-a*0.15, p.y);
    return length8(q)-t.y;
}

float cyl(vec2 p, float r) { return length(p) - r; }

float glow = 0.0, glow2 = 0.0, glow3 = 0.0;

float DistanceToObject(vec3 p) {
    vec3 orig = p;
    p.yz = mapAbyssalSpiral(p.yz);
    p = mix(orig, p, animStructure);

    const float outerRad = 4.0;
    float lenXY = length(p.xy);
    float final = lenXY - outerRad;
    final = max(final, -(lenXY - (outerRad-0.8)));

    float slice = 0.05;
    vec3 grid = -abs(fract(p*0.9)-0.5) + slice;
    final = max(final, grid.z);

    vec3 rep = fract(p*1.1)-0.5;
    float scale = 1.0;
    float mult = 0.35;
    for (int i = ZERO_TRICK; i < 4; i++) {
        float uglyDivider = max(1.0, float(i));
        float dist = cyl(rep.xz/scale, mult/scale)/uglyDivider;
        final = max(final, -dist);
        dist = cyl(rep.xy/scale, mult/scale)/uglyDivider;
        final = max(final, -dist);
        dist = cyl(rep.yz/scale, mult/scale)/uglyDivider;
        final = max(final, -dist);
        scale *= 1.2 + sin(localTime*0.2)*0.1;
        rep = fract(rep*scale) - 0.5;
    }

    vec3 sp = p;
    sp.x = abs(sp.x)-6.0;
    sp.z = fract(sp.z*1.2) - 0.5;
    float struts = sdBox(sp+vec3(3.2, 0.15-sin(sp.x*1.5 + localTime*0.3)*1.2, 0.0), vec3(1.8, 0.06, 0.03))*0.5;
    glow3 += 0.00008/max(0.015, struts);
    final = min(final, struts);

    rep.yz = fract(p.yz*1.1)-0.5;
    rep.x = p.x;
    scale = 1.2;
    float jolt = max(0.0, sin(length(orig.yz)*0.8 + localTime*15.0))*0.85;
    jolt *= saturate(0.4-pulse);
    float spiral = sdBox(RotateX(rep+vec3(-0.06,0.0,0.0), pulse), vec3(0.015+jolt*0.5,1.1, mult*0.015)/scale);
    glow3 += 0.002/max(0.003,spiral);
    final = min(final, spiral + (1.0-animStructure) * 100.0);

    vec3 rp = mapVoidInvert(p.xzy);
    rp.x = -abs(rp.x);
    rp.y = fract(rp.y*1.3) - 0.5;
    float torus = sdTorusWraith(rp + vec3(3.5, 0.0, 0.0), vec2(0.25, 0.0004), p.z);
    glow2 += 0.002/max(0.04, torus);
    final = min(final, torus);

    glow += (0.025+abs(sin(orig.x-localTime*2.5)*0.12)*jolt)/length(orig.yz + vec2(sin(localTime*0.5), cos(localTime*0.5))*0.5);

    return final;
}

vec3 RayTrace(in vec2 fragCoord) {
    glow = 0.0;
    glow2 = 0.0;
    glow3 = 0.0;

    float slt = sin(localTime*0.8);
    float stepLike = pow(abs(slt), 0.9)*sign(slt);
    pulse = stepLike*PI/5.0 + PI/5.0;

    vec2 uv = fragCoord.xy/u_resolution.xy * 2.0 - 1.0;

    vec3 camPos, camUp, camLookat;
    const float t0 = 0.0, t1 = 10.0, t2 = 18.0, t3 = 27.0, t4 = 42.0, t5 = 50.0, t6 = 75.0;
    localTime = mod(u_time, t6); // 循环动画
    
    #ifdef MANUAL_CAMERA
    // 鼠标控制相机
    if (u_mouse.x > 0.0 || u_mouse.y > 0.0) {
        camPos = vec3(
            cos(u_mouse.y/u_resolution.y*PI)*cos(u_mouse.x/u_resolution.x*PI*2.0)*9.0,
            sin(u_mouse.y/u_resolution.y*PI),
            cos(u_mouse.y/u_resolution.y*PI)*sin(u_mouse.x/u_resolution.x*PI*2.0)*9.0
        );
        camUp = vec3(0,1,0);
        camLookat = vec3(0,0,0);
        animStructure = 1.0;
        fade = 1.0;
    } else 
    #endif
    {
        // 恢复原始时间线相机动画
        if (localTime < t1) {
            animStructure = 0.0;
            float time = localTime - t0;
            float alpha = time / (t1 - t0);
            fade = saturate(time) * saturate(t1 - localTime);
            camPos = vec3(60.0, -3.0, 2.0);
            camPos.x -= alpha * 8.0;
            camUp = vec3(0,1,0);
            camLookat = vec3(50,0.0,0);
        } else if (localTime < t2) {
            animStructure = 0.0;
            float time = localTime - t1;
            float alpha = time / (t2 - t1);
            fade = saturate(time) * saturate(t2 - localTime);
            camPos = vec3(14.0, 4.0, -1.0);
            camPos.x -= smoothstep(0.0, 1.0, alpha) * 5.5;
            camUp = vec3(0,1,0);
            camLookat = vec3(0,6.0,-1.0);
        } else if (localTime < t3) {
            animStructure = 1.0;
            float time = localTime - t2;
            float alpha = time / (t3 - t2);
            fade = saturate(time) * saturate(t3 - localTime);
            camPos = vec3(14.0, 7.0, -1.0);
            camPos.y -= alpha * 2.0;
            camPos.x = cos(alpha*1.2) * 7.0;
            camPos.z = sin(alpha*1.2) * 7.0;
            camUp = normalize(vec3(0,1,-0.4 - alpha * 0.6));
            camLookat = vec3(0,0.0,-1.0);
        } else if (localTime < t4) {
            animStructure = 1.0;
            float time = localTime - t3;
            float alpha = time / (t4 - t3);
            fade = saturate(time) * saturate(t4 - localTime);
            camPos = vec3(14.0, 3.5, -3.0);
            camPos.y -= alpha * 2.0;
            camPos.x = cos(alpha*1.2) * 7.2 - alpha*0.3;
            camPos.z += sin(alpha*1.2) * 7.2 - alpha*0.3;
            camUp = normalize(vec3(0,1,0.0));
            camLookat = vec3(0,0.0,0.0);
        } else if (localTime < t5) {
            animStructure = 1.0;
            float time = localTime - t4;
            float alpha = time / (t5 - t4);
            fade = saturate(time) * saturate(t5 - localTime);
            camPos = vec3(0.0, -8.0, -1.2);
            camPos.y -= alpha * 2.0;
            camPos.x = cos(alpha*1.2) * 1.8 - alpha*1.8;
            camPos.z += sin(alpha*1.2) * 1.8 - alpha*1.8;
            camUp = normalize(vec3(0,1,0.0));
            camLookat = vec3(0,-3.5,0.0);
        } else {
            float time = localTime - t5;
            float alpha = time / (t6 - t5);
            float smoothv = smoothstep(0.0, 1.0, saturate(alpha*2.0-0.15));
            animStructure = 1.0-smoothv;
            fade = saturate(time) * saturate(t6 - localTime);
            camPos = vec3(12.0, -1.2+smoothv*1.2, 0.0);
            camPos.x -= alpha * 8.0;
            camUp = normalize(vec3(0,1.0-smoothv,0.0+smoothv));
            camLookat = vec3(0,0.0,0.0);
        }
    }

    vec3 camVec = normalize(camLookat - camPos);
    vec3 sideNorm = normalize(cross(camUp, camVec));
    vec3 upNorm = cross(camVec, sideNorm);
    vec3 worldFacing = camPos + camVec;
    vec3 worldPix = worldFacing + uv.x * sideNorm * (u_resolution.x/u_resolution.y) + uv.y * upNorm;
    vec3 rayVec = normalize(worldPix - camPos);

    float dist = 1.0;
    float t = 0.1 + Hash2d(uv)*0.12;
    const float maxDepth = 50.0;
    vec3 pos = vec3(0,0,0);
    const float smallVal = 0.0005;
    for (int i = ZERO_TRICK; i < 220; i++) {
        pos = (camPos + rayVec * t).yzx;
        dist = DistanceToObject(pos);
        dist = min(dist, length(pos.yz));
        t += dist;
        if (t > maxDepth || abs(dist) < smallVal) break;
    }

    float glowSave = glow;
    float glow2Save = glow2;
    float glow3Save = glow3;
    vec3 sunDir = normalize(vec3(0.9, 1.2, -1.8));
    vec3 finalColor = vec3(0.0);

    if (t <= maxDepth) {
        vec3 smallVec = vec3(smallVal, 0, 0);
        vec3 normalU = vec3(dist - DistanceToObject(pos - smallVec.xyy),
                           dist - DistanceToObject(pos - smallVec.yxy),
                           dist - DistanceToObject(pos - smallVec.yyx));
        vec3 normal = normalize(normalU);

        float ambientS = 1.0;
        ambientS *= saturate(DistanceToObject(pos + normal * 0.06)*18.0);
        ambientS *= saturate(DistanceToObject(pos + normal * 0.12)*9.0);
        ambientS *= saturate(DistanceToObject(pos + normal * 0.24)*4.5);
        ambientS *= saturate(DistanceToObject(pos + normal * 0.48)*2.25);
        float ambient = ambientS * saturate(DistanceToObject(pos + normal * 1.8)*1.125);
        ambient = saturate(ambient);

        float sunShadow = 1.0;
        float iter = 0.012;
        vec3 nudgePos = pos + normal*0.0025;
        for (int i = ZERO_TRICK; i < 35; i++) {
            float tempDist = DistanceToObject(nudgePos + sunDir * iter);
            sunShadow *= saturate(tempDist*140.0);
            if (tempDist <= 0.0) break;
            iter += max(0.012, tempDist)*1.0;
            if (iter > 4.5) break;
        }
        sunShadow = saturate(sunShadow);

        float n = 0.0;
        n += noise(pos*40.0)*0.5;
        n += noise(pos*80.0)*0.25;
        n += noise(pos*160.0)*0.125;
        n += noise(pos*320.0)*0.0625;
        n *= 0.9;
        normal = normalize(normal + (n-1.8)*0.12);

        vec3 texColor = vec3(0.3, 0.35, 0.4);
        vec3 decay = vec3(0.2, 0.15, 0.1) - noise(pos*100.0)*0.3;
        texColor *= smoothstep(texColor, decay, vec3(saturate(DecayNoise3D(pos*10.0))-0.25));

        texColor *= vec3(1.0)*n*0.06;
        texColor *= 0.6;
        texColor = saturate(texColor);

        vec3 lightColor = vec3(1.8) * saturate(dot(sunDir, normal)) * sunShadow;
        float ambientAvg = (ambient*2.5 + ambientS) * 0.3;
        lightColor += vec3(0.5, 0.1, 0.2) * saturate(-normal.z *0.6+0.5)*pow(ambientAvg, 0.4);
        lightColor += vec3(0.05, 0.2, 0.5) * saturate(normal.y *0.6+0.5)*pow(ambientAvg, 0.4);
        lightColor += vec3(0.1, 0.2, 0.5) * saturate(dot(-pos, normal))*pow(ambientS, 0.35);
        lightColor *= 3.5;

        finalColor = texColor * lightColor;
    }

    float center = length(pos.yz);
    finalColor += vec3(0.1, 0.2, 0.5) * glowSave*1.0;
    finalColor += vec3(0.5, 0.2, 0.1) * glow2Save*1.0;
    finalColor += vec3(0.15, 0.2, 0.5) * glow3Save*1.8;

    finalColor *= vec3(1.0) * saturate(1.0 - length(uv/2.0));
    finalColor *= 0.9;

    return vec3(clamp(finalColor, 0.0, 1.0)*saturate(fade+0.2));
}

void main() {
    localTime = u_time;
    vec2 fragCoord = gl_FragCoord.xy;
    vec3 color = RayTrace(fragCoord);
    gl_FragColor = vec4(sqrt(clamp(color, 0.0, 1.0)), 1.0);
}