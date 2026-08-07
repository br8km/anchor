# Keep entry metadata keys free-form

`anchor` does not impose a canonical set of metadata keys inside secret entries. We chose free-form key/value lines because users may want to store arbitrary fields without app coupling, and because a fixed schema would make the file format harder to extend without adding hidden product policy.
