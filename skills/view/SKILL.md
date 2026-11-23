---
name: view
description: Open the native viewer for a project's knowledge graph (.ua/knowledge-graph.json) to explore its layers, nodes and tour
argument-hint: "[path/to/knowledge-graph.json]"
---

# /ua-viewer:view

Open the knowledge graph in the native viewer.

## Steps

1. Resolve the graph path. If the user gave an argument, use it. Otherwise use `.ua/knowledge-graph.json` in the current project. If the file does not exist, tell the user to run `/understand` first and stop.
2. Launch the viewer in the background. The launcher picks the right binary for this OS and CPU, downloads it from the matching GitHub Release if needed, or builds it from source as a fallback:
   ```bash
   bash "${CLAUDE_PLUGIN_ROOT}/scripts/launch.sh" "<graph path>"
   ```
3. Tell the user the window is open and summarise the controls:
   - Drag the background to pan. Scroll or use the + / − buttons to zoom. "Quay lại" and "Đi tiếp" move between the overview and layers.
   - Click a layer card or a layer in the left list to open it. Click a node to select it.
   - Press "Chi tiết" in the top bar to open the details panel. Double-click a node to open its source. Use "Mở" to open another knowledge-graph.json or project folder, or drag a file into the window.
   - Use "Tùy chọn > Tour" to run a guided tour. Use the search box at the top right (or press /) to find a node. Press `F` to fit the graph to the window.
   - Tick "Hiện mọi cạnh" in the "Tùy chọn" menu to draw every edge.

Do not modify the graph file.
