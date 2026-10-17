<script setup lang="ts">
import { ref } from 'vue';
import UiDialog from './Dialog.vue';
import UiProgressBar from './ProgressBar.vue';
import sample1 from '../../assets/wabi/sample_1_ai_creation_liquid_glass.png';
import sample2 from '../../assets/wabi/sample_2_bili_parse_liquid_glass.png';
import sample3 from '../../assets/wabi/sample_3_concat_editor_liquid_glass.png';
import sample4 from '../../assets/wabi/sample_4_settings_liquid_glass.png';
import sample5 from '../../assets/wabi/sample_5_download_queue_liquid_glass.png';
import sample6 from '../../assets/wabi/sample_6_wabi_spec_modal.png';
const open = defineModel<boolean>({ default: false });
const displacement = ref(14);
const selected = ref(0);
const samples = [
  { src: sample1, title: 'AI 创作工作台' },
  { src: sample2, title: '链接解析与多 P' },
  { src: sample3, title: '独立剪辑桥接' },
  { src: sample4, title: '设置与服务验证' },
  { src: sample5, title: '下载队列与诊断' },
  { src: sample6, title: 'Wabi 视觉规范' },
];
</script>
<template>
  <svg class="wabi-filter-defs" aria-hidden="true" focusable="false">
    <defs>
      <filter id="wabi-liquid-refract" x="-15%" y="-15%" width="130%" height="130%" color-interpolation-filters="sRGB">
        <feTurbulence type="fractalNoise" baseFrequency="0.014 0.022" numOctaves="2" seed="19" result="noise" />
        <feGaussianBlur in="noise" stdDeviation="2.2" result="smoothedNoise" />
        <feDisplacementMap
          in="SourceGraphic"
          in2="smoothedNoise"
          :scale="displacement"
          xChannelSelector="R"
          yChannelSelector="G"
        />
      </filter>
      <linearGradient id="wabi-ribbon" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0%" stop-color="#0d7a68" stop-opacity=".02" />
        <stop offset="50%" stop-color="#2dd4bf" stop-opacity=".22" />
        <stop offset="100%" stop-color="#0d7a68" stop-opacity=".04" />
      </linearGradient>
    </defs>
  </svg>
  <svg class="wabi-ambient" viewBox="0 0 1440 900" preserveAspectRatio="none" aria-hidden="true" focusable="false">
    <g filter="url(#wabi-liquid-refract)">
      <path
        d="M-80 120 C300 80 430 300 820 170 S1260 40 1520 180 L1520 390 C1140 210 950 470 570 340 S160 180 -80 300Z"
        fill="url(#wabi-ribbon)"
      />
      <path
        d="M-80 650 C320 460 620 840 1010 650 S1270 580 1520 720 L1520 940 C1100 780 820 960 390 800 S100 810 -80 880Z"
        fill="url(#wabi-ribbon)"
      />
    </g>
  </svg>
  <UiDialog
    v-model="open"
    title="Wabi 液体玻璃方案与样例图"
    description="Graphite Slate & Emerald Teal · CutFlow 0.5.0"
    size="wide"
  >
    <div class="wabi-spec">
      <div class="wabi-spec-intro">
        <span class="wabi-spec-kicker">光学材质 · 真实任务</span>
        <h2>让光流动，让创作专注。</h2>
        <p>石墨雾画布、青绿操作焦点与双层高光倒角。折射仅作用于装饰光带，保持正文与控件清晰。</p>
      </div>
      <div class="wabi-spec-controls">
        <div>
          <label for="wabi-displacement">折射位移强度</label
          ><output for="wabi-displacement">{{ displacement }} px</output>
        </div>
        <input id="wabi-displacement" v-model.number="displacement" type="range" min="0" max="32" step="1" />
        <p>拖动滑块观察背景与下方透镜的位移变化。</p>
        <div class="wabi-lens" aria-hidden="true"><i /></div>
        <div class="wabi-progress-demo">
          <span>下载流动效果 · 演示 68%</span><UiProgressBar :value="68" status="downloading" label="液体进度条演示" />
        </div>
      </div>
      <div class="wabi-spec-palette">
        <span>60% 石墨雾画布</span><span>30% 液态玻璃结构</span><span>10% 翡翠青绿强调</span>
      </div>
      <figure class="wabi-sample">
        <img
          :src="samples[selected].src"
          :alt="samples[selected].title + ' · 用户提供的设计参考'"
          width="1440"
          height="900"
        />
        <figcaption>{{ samples[selected].title }} · 设计参考，图中任务与媒体为原型演示内容</figcaption>
      </figure>
      <div class="wabi-gallery" aria-label="选择设计参考图">
        <button
          v-for="(sample, index) in samples"
          :key="sample.src"
          type="button"
          :aria-pressed="selected === index"
          @click="selected = index"
        >
          <img :src="sample.src" alt="" width="1440" height="900" loading="lazy" /><span>{{ sample.title }}</span>
        </button>
      </div>
      <p class="wabi-spec-note">
        下载任务按真实状态显示：下载中流动，暂停呈琥珀色，失败呈红色，完成后静止。遵循系统减少动态效果偏好。
      </p>
    </div>
  </UiDialog>
</template>
