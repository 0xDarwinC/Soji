- [x] core functionality (add stickers to db and select them on click)
- [x] tabs
- [x] refresh button next to settings button (calls refresh_library)
- [x] settings
    - [x] Switch from acrylic to mica for performance
    - [x] Wipe recency/history cache (with an extra confirmation)
    - [x] Wipe favorites (with an extra confirmation)
    - [x] Wipe db (remove the database index, dont actually wipe the pictures.)
    - [x] Set sticker directory
    - [x] Change size of recents cache (from 18 to whatever)
    - [x] Change number of displayed elements in a tab (-1 to set to max possible)
    - [x] Disable animations
    - [x] Run on startup (default true)
    - [x] Check for updates/auto update
    - [x] Restore defaults
- [x] favorites (and should dynamically update)
- [x] recents
- [x] resize stickers properly to common sizes (discord uses 160x160), if a sticker is smaller than that maybe we shouldn't upscale to preserve resolution.
    - [x] if square, then 160x160
    - [x] if wider than tall, then preserve aspect ratio but height 160
    - [x] else preserve ratio but width 160
- [x] while adding new images to db, theres a load screen that shows progress
- [x] add close button
- [x] backup and restore the user's clipboard post paste
    - Note: Should backup and restore most formats but NOT all. Does a byte read.
- [x] drag drop your own stickers, auto convert to supported file type
- [x] rename and change sticker classification dynamically (move it to a different pack) Make sure to keep the recency/history. 
    - [x] right click menu that can add fav or edit or delete
    - [x] pencil button under heart that can edit the sticker
    - [x] change pack while keeping its history
- [x] hovering sticker buttons for feedback, perhaps sfx
- [x] make window resizable
- [x] optimize and clean up the repository for performance. Needs to support a large amount of photos.
    - [x] Make async where it makes sense, dont have thread pool exhaustion
    - [x] Only render what you need. But the objects need to exist for search
    - [x] clean up the repo
    - [ ] add documentation
- [x] make window be an offset from cursor like emoji keyboard
    - [x] Perhaps different behavior if cursor not in a textbox.
    - [ ] If user opens settings, or any edit interaction, switches to workspace mode
- [ ] gif resizing
  - [ ] needs to resize with a similar intuition of how we resize images in terms of dimensions
  - [ ] need to use rust-ffmpeg or some other derivative
- [ ] help button 
    - [ ] a one-page tutorial that shows how to set it up
    - [ ] credits, contributors, and donate button
- [ ] system tray features
    - [ ] closing window should minimize to system tray
    - [ ] system tray with soji logo
- [ ] improve ui
    - [x] fix scrollbar for tags
    - [ ] graphics when no results, empty sticker folder, etc
    - [ ] make dialogs styled (deletion, reindexing, renaming, updates)
    - [ ] our own images instead of emojis for buttons
    - [ ] remove text from titlebar
- [ ] release and setup executable. user should be able to determine their settings in this step.
    - [x] Pre release and setup executable.
    - [ ] Needs to be able to configure following settings:
        - [ ] low power mode (disables anims and mica)
        - [ ] target dir
        - [ ] Run on startup (recommended) 
- [ ] graphics
    - [ ] soji logo (undecided)
    - [ ] no results
    - [ ] help page infographics
    - [ ] demonstration video/gif with that zoomy effect thing
    - [ ] youtube video product demo

Bugs:
- [ ] Update dialog text is not correct
- [ ] release yml optimizations not working
- [ ] fuzzy search needs to be improved
    - [ ] instead of just fuzzy, match any keys
- [ ] remove ability to highlight text within app
- [ ] when drag/drop or ctrlv a new image to open editor, window doesnt resize to default
 

Test:
- [ ] Move method destination outside of user dir subtree
- [ ] Check for updates works...
    - [ ] Notify on hourly
- [ ]  drag and drop
    - [ ] large files >25mb
    - [ ] file types (unsupported vs supp)
- [ ] app interactions with multi monitor setup


Post-Release Features:
- [ ] notification service for edit actions and refresh
- [ ] sort by recency score and A-Z (default)
- [ ] sticker editor
    - [ ] see pjsk sticker editor for details
    - [ ] move text x and y axis, rotate full 360
    - [ ] additional font options?
- [ ] animated formats conversion using vid2gif (ffmpeg wrapper) or other crates
    - [ ] if animated webp, then gif
    - [ ] if webm, then gif
    - [ ] other formats as necessary
- [ ] Masonry Layout option
- [ ] Pretext integration for efficient layout calcs



Potential features:
- [ ] gpu acceleration for image indexing?