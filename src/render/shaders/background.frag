#ifdef GL_ES
precision mediump float;
#endif

uniform vec2 u_resolution;
uniform float u_time;

void main() {
    vec2 u = gl_FragCoord.xy / u_resolution.xy;

    vec3 col = 0.5 + 0.5*cos(u_time+u.xyx+vec3(0,2,4));

    gl_FragColor = vec4(col, 1.0);
}
