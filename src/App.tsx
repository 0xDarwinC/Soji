import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface Sticker {
  name: string;
  path: string;
  format: string;
}

function App() {
  const [stickers, setStickers] = useState<Sticker[]>([]);
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    loadStickers("");
    
    // Auto-focus input when window gains focus (Alt + .)
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        if (focused) {
            setTimeout(() => {
                inputRef.current?.focus();
                // Optional: Select all text so you can overwrite previous search immediately
                inputRef.current?.select();
            }, 50);
        }
    });

    return () => {
        unlisten.then(f => f());
    }
  }, []);

  const loadStickers = async (searchQuery: string) => {
    try {
        const result = await invoke<Sticker[]>("search_stickers", { query: searchQuery });
        setStickers(result);
    } catch (err) {
        console.error(err);
    }
  };

  const handleSearch = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setQuery(val);
    loadStickers(val);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // enter sends first sticker in list
    if (e.key === "Enter" && stickers.length > 0) {
        handleStickerClick(stickers[0].path);
    }
    // kicks you out
    if (e.key === "Escape") {
        invoke("hide_window").catch(() => {});
    }
  };

  const handleStickerClick = async (path: string) => {
    try {
      await invoke("select_sticker", { path });
    } catch (error) {
      console.error("Failed to select sticker:", error);
    }
  };

  return (
    <div style={{ 
      padding: "20px", 
      height: "100%", 
      width: "100%",
      overflowX: "hidden",
      overflowY: "hidden",
      boxSizing: "border-box",
      background: "transparent",
      display: "flex",
      flexDirection: "column",
    }}>
      <h1 style={{ marginBottom: "20px" }}>Soji</h1>
      {/* SEARCH BAR */}
      <input 
        ref={inputRef}
        type="text" 
        placeholder="Search stickers..." 
        value={query}
        onChange={handleSearch}
        onKeyDown={handleKeyDown}
        style={{
            width: "100%",
            padding: "12px",
            marginBottom: "15px",
            borderRadius: "8px",
            border: "1px solid rgba(255,255,255,0.2)",
            background: "rgba(0,0,0,0.3)",
            color: "white",
            fontSize: "16px",
            outline: "none",
            backdropFilter: "blur(10px)"
        }}
      />

      {/* THE GRID (Scrollable Area) */}
      <div style={{ 
        flex: 1, // Fill remaining space
        overflowX: "hidden",
        overflowY: "auto",
        display: "grid", 
        gridTemplateColumns: "repeat(auto-fill, minmax(100px, 1fr))", 
        gridAutoRows: "min-content",
        gap: "15px",
        paddingBottom: "10px"
      }}>
        {stickers.map((s, index) => (
          <div key={s.path} 
          onClick={() => handleStickerClick(s.path)}
          style={{ 
            display: "flex", 
            flexDirection: "column", 
            alignItems: "center",
            padding: "10px",
            background: index === 0 && query.length > 0 ? "rgba(255, 255, 255, 0.3)" : "rgba(255, 255, 255, 0.1)", // Highlight top result
            borderRadius: "8px",
            backdropFilter: "blur(5px)",
            cursor: "pointer",
            transition: "background 0.2s"
          }}
          onMouseEnter={(e) => e.currentTarget.style.background = "rgba(255, 255, 255, 0.2)"}
          onMouseLeave={(e) => e.currentTarget.style.background = index === 0 && query.length > 0 ? "rgba(255, 255, 255, 0.3)" : "rgba(255, 255, 255, 0.1)"}
          >
            <img 
              src={convertFileSrc(s.path)}
              alt={s.name}
              style={{ 
                width: "80px", 
                height: "80px", 
                objectFit: "contain", 
                marginBottom: "10px" 
              }} 
            />
            
            <span style={{ fontSize: "12px", textAlign: "center", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: "100%" }}>
              {s.name}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

export default App;