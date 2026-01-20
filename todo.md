- [x] core functionality (add stickers to db and select them on click)
- [x] tabs
- [x] refresh button next to settings button (calls refresh_library)
- [ ] help button 
    - [ ] a one-page tutorial that shows how to set it up
    - [ ] credits, contributors, and donate button
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
- [ ] drag drop your own stickers, auto convert to supported file type
- [x] rename and change sticker classification dynamically (move it to a different pack) Make sure to keep the recency/history. 
    - [x] right click menu that can add fav or edit or delete
    - [x] pencil button under heart that can edit the sticker
    - [x] change pack while keeping its history
- [x] hovering sticker buttons for feedback, perhaps sfx
- [ ] make window be an offset from cursor like emoji keyboard
    - [ ] Perhaps different behavior if cursor not in a textbox.
- [x] make window resizable
- [ ] system tray features
    - [ ] closing window should minimize to system tray
    - [ ] system tray with soji logo
- [x] optimize and clean up the repository for performance. Needs to support a large amount of photos.
    - [x] Make async where it makes sense, dont have thread pool exhaustion
    - [x] Only render what you need. But the objects need to exist for search
    - [x] clean up the repo
    - [ ] add documentation
- [ ] improve ui
    - [x] fix scrollbar for tags
    - [ ] graphics when no results, empty sticker folder, etc
    - [ ] make dialogs styled (deletion, reindexing, renaming, updates)
    - [ ] our own images instead of emojis for buttons
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
- [x] Clipboard restoration will not support ALL file types. Need to exhaustively search
for unsupported types and fix them.
- [x] Move pack menu and settings menu has right click context menu -- need to disable
- [ ] Check for updates text result doesnt show immediately, needs to scroll to fit into window
- [ ] Display limit can't go to -1, also hard to enter values

Test:
- [ ] Move method destination outside of target subtree
- [ ] Images smaller than 160x160
- [ ] Check for updates works...
    - [x] Check for update manually
    - [ ] Notify on startup
    - [ ] Notify on hourly


Post-Release Features:
- [ ] notification service for edit actions and refresh

Potential features:
- [ ] Resize gif
    - Not feasible until I find a smart implementation. FIR resizing each frame of a gif can become computationally expensive really fast.
- [ ] gpu acceleration for image indexing?