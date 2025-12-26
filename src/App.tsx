import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";

interface Sticker {
  name: string;
  path: string;
  format: string;
}

function App() {
  const [stickers, setStickers] = useState<Sticker[]>([]);

  useEffect(() => {
    invoke<Sticker[]>("list_stickers")
      .then((result) => setStickers(result))
      .catch((err) => console.error(err));
  }, []);

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
      overflowY: "auto",
      boxSizing: "border-box",
      background: "transparent", 
    }}>
      <h1 style={{ marginBottom: "20px" }}>Soji</h1>
      
      {/* THE GRID */}
      <div style={{ 
        display: "grid", 
        gridTemplateColumns: "repeat(auto-fill, minmax(100px, 1fr))", 
        gap: "15px" 
      }}>
        {stickers.map((s) => (
          <div key={s.path} 
          onClick={() => handleStickerClick(s.path)}
          style={{ 
            display: "flex", 
            flexDirection: "column", 
            alignItems: "center",
            padding: "10px",
            background: "rgba(255, 255, 255, 0.1)", // Semi-transparent card
            borderRadius: "8px",
            backdropFilter: "blur(5px)" // Extra blur for style
          }}>
            {/* IMAGE PREVIEW */}
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