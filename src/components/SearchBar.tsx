import React, { forwardRef } from 'react';

interface SearchBarProps {
    query: string;
    onSearch: (value: string) => void;
    onKeyDown: (e: React.KeyboardEvent) => void;
}

export const SearchBar = forwardRef<HTMLInputElement, SearchBarProps>(({ query, onSearch, onKeyDown }, ref) => {
    return (
        <input
            ref={ref}
            type="text"
            placeholder="Search stickers..."
            value={query}
            onChange={(e) => onSearch(e.target.value)}
            onKeyDown={onKeyDown}
            style={{
                width: "100%",
                padding: "12px",
                marginBottom: "10px",
                borderRadius: "10px",
                border: "1px solid rgba(255,255,255,0.15)",
                background: "rgba(0,0,0,0.3)",
                color: "white",
                fontSize: "16px",
                outline: "none",
                backdropFilter: "blur(10px)",
                boxSizing: "border-box"
            }}
            autoFocus
        />
    );
});