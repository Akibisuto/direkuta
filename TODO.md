# TODO

## Before v0.1.0 Release

- [x] ~~Allow middleware to reject a request.~~
  - ~~Mostly for authorization and the like.~~
- [ ] Tests.
- [x] ~~Change router to support a `path` function.~~
- [x] ~~Implement middleware calling.~~
- [x] Route parameter ids instead of tuple positions.
- [ ] Static files with path sanitization.
- [X] ~~Error handling~~
- [ ] Response builders
  - [ ] Files
    - [ ] Code
      - [X] CSS
      - [ ] HTML
      - [X] JS
    - [ ] Images
      - [ ] GIF
      - [ ] JPEG
      - [ ] PNG
    - [ ] Sound
      - [ ] FLAC
      - [ ] MP3
      - [ ] Ogg
    - [ ] Video
      - [ ] MP4
      - [ ] WebM
  - [x] JSON
  - [ ] Tera
- [ ] Large File Streaming
  - futures-fs/tokio-fs and Hyper `Body::pair`?

## Future

- [ ] Documentation, both in the code and the readme.
- [ ] Make Direkuta faster (somehow).
