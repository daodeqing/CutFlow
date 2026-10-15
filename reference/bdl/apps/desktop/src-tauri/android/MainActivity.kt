package com.yueli.bdl

import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    hideStatusBar()
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    // The default 8 CSS px floor inflates labels in high-density tablet layouts.
    // Let the responsive frontend own font sizes while retaining system text zoom.
    webView.settings.minimumFontSize = 1
    webView.settings.minimumLogicalFontSize = 1
  }

  override fun onWindowFocusChanged(hasFocus: Boolean) {
    super.onWindowFocusChanged(hasFocus)
    if (hasFocus) hideStatusBar()
  }

  private fun hideStatusBar() {
    WindowCompat.getInsetsController(window, window.decorView).apply {
      systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
      hide(WindowInsetsCompat.Type.statusBars())
    }
  }
}
