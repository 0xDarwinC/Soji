import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core"; // <--- IMPORT THIS

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

  return (
    <div style={{ 
      padding: "20px", 
      height: "100vh", 
      overflowY: "auto", // Allow scrolling inside the div, not the window
      // Add a slight dark tint so text is readable on light wallpapers
      background: "rgba(0, 0, 0, 0.2)" 
    }}>
      <h1 style={{ marginBottom: "20px" }}>Soji Debug</h1>
      
      {/* THE GRID */}
      <div style={{ 
        display: "grid", 
        gridTemplateColumns: "repeat(auto-fill, minmax(100px, 1fr))", 
        gap: "15px" 
      }}>
        {stickers.map((s) => (
          <div key={s.path} style={{ 
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
              src={convertFileSrc(s.path)} // <--- CONVERT PATH HERE
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