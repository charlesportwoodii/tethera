<script lang="ts">
  interface Props {
    name: string;
    source: string;
    truncated?: boolean;
  }

  let { name, source, truncated = false }: Props = $props();
</script>

<!--
  An HTML file shown as the page it is, rather than as its source.

  **`sandbox` with no tokens is the whole of the safety here, and it is not
  decoration.** This app runs with `csp: null` and draws agent transcripts, so a
  document that arrived over the wire is the highest-value target it has. An
  empty `sandbox` puts the frame in an opaque origin with scripting off: the
  document cannot reach this app's DOM, its storage, its Tauri bridge, or the
  page it is drawn in. Adding `allow-scripts` gives that back in part, and
  `allow-scripts allow-same-origin` together defeat the sandbox entirely - the
  frame can then reach out and rewrite its parent, which is the one combination
  that must never be written here.

  The cost is honest and worth stating: a page whose layout is built by script
  renders without it. A design mock draws its decorative canvases and nothing
  else goes missing.

  Subresources still load. A document that links a remote stylesheet or font
  fetches it, so opening one tells that host the file was read. Sandboxing is
  not a privacy boundary and does not pretend to be.
-->
<div class="html-preview">
  <iframe class="html-preview__frame" title="Rendered preview of {name}" sandbox="" srcdoc={source}
  ></iframe>

  {#if truncated}
    <span class="html-preview__truncated">first part only · save the file to read it all</span>
  {/if}
</div>

<style lang="scss">
  .html-preview {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
  }

  .html-preview__frame {
    flex: 1;
    min-height: 0;
    width: 100%;
    border: 0;
    border-radius: 10px;
    // The document brings its own colours and mostly assumes it is on a page of
    // its own. White is what it would get in a browser.
    background: #fff;
  }

  .html-preview__truncated {
    flex: none;
    font-size: 11px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    opacity: 0.7;
  }
</style>
