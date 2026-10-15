import { diagnosticsEnvironment } from '../api/tauri'

export const captureDiagnosticEnvironment = async (): Promise<string> => {
  const userAgent = globalThis.navigator?.userAgent ?? '未提供'
  try {
    const info = await diagnosticsEnvironment()
    return [
      '当前环境:',
      `BDL: ${info.app_version}`,
      `系统: ${info.os}${info.device.os_version ? ` ${info.device.os_version}` : ''} · ${info.arch}`,
      ...(info.device.android_sdk != null ? [`Android SDK: ${info.device.android_sdk}`] : []),
      ...(info.device.webview_version ? [`WebView: ${info.device.webview_version}`] : []),
      ...(!info.device_info_available ? ['原生系统详情: 未能获取'] : []),
      `当前下载设置: 分段 ${info.segment_count} · 并发 ${info.concurrent_tasks} · 重试 ${info.retry_count} · 应用代理${info.proxy_enabled ? '开启' : '关闭'}`,
      `WebView UA: ${userAgent}`,
    ].join('\n')
  } catch {
    return `当前环境: 未能获取原生信息\nWebView UA: ${userAgent}`
  }
}
