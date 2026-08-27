package com.alaydriem.tethera

import android.content.Context
import android.graphics.Color
import android.os.Bundle
import android.view.ViewGroup
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  // Implemented in Rust. See the JNI export in src/lib.rs for why this exists at
  // all: tao 0.35 stopped initializing ndk-context, so nothing else populates it.
  private external fun initNdkContext(context: Context)

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // Before anything Rust-side runs. The keyring store and iroh's network
    // watcher both read the Android context on first use, and both abort the
    // process if it is absent.
    initNdkContext(this.applicationContext)
  }

  override fun onWebViewCreate(webView: WebView) {
    // Transparent, so the barcode scanner's camera preview is visible. The
    // plugin draws that preview *behind* the webview rather than into it, so an
    // opaque webview yields a camera that is demonstrably running and a screen
    // that shows nothing. The page paints its own background instead; see
    // app.scss, which drops it for the duration of a scan.
    webView.setBackgroundColor(Color.TRANSPARENT)

    ViewCompat.setOnApplyWindowInsetsListener(webView) { view, windowInsets ->
      // Both types, so each edge takes whichever is deeper. The status bar alone
      // is not enough on a device whose camera cutout is deeper than it.
      val types =
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      val insets = windowInsets.getInsets(types)

      // A margin, not padding. Padding lives inside the view's own bounds, so
      // Chromium still lays the page out against the full window and the
      // reservation is invisible to CSS - the title then renders under the
      // clock, which is exactly what happened before this listener existed. A
      // margin makes the view itself smaller, and the viewport is the view.
      val params = view.layoutParams as? ViewGroup.MarginLayoutParams

      if (
        params != null &&
        (
          params.topMargin != insets.top ||
          params.bottomMargin != insets.bottom ||
          params.leftMargin != insets.left ||
          params.rightMargin != insets.right
        )
      ) {
        params.setMargins(insets.left, insets.top, insets.right, insets.bottom)
        view.layoutParams = params
      }

      // Installing a listener replaces the view's own onApplyWindowInsets, so
      // the result still has to be handed to it or Chromium is cut out of inset
      // handling entirely, the on-screen keyboard included.
      val consumed = WindowInsetsCompat.Builder(windowInsets)
        .setInsets(types, Insets.NONE)
        .build()

      ViewCompat.onApplyWindowInsets(view, consumed)
    }
  }
}
