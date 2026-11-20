# Included after every project() of CMake-built dependencies (whisper.cpp) via
# CMAKE_PROJECT_INCLUDE, set in .cargo/config.toml.
#
# With MSVC the cmake crate replaces CMake's per-configuration flags with the
# cc crate's flags, which carry no optimization, so Release builds compile with
# /Od. Restore the optimization CMake would normally add. Other compilers get
# their flags right and are left alone.
if(MSVC)
    string(APPEND CMAKE_C_FLAGS_RELEASE " /O2 /Ob2 /DNDEBUG")
    string(APPEND CMAKE_CXX_FLAGS_RELEASE " /O2 /Ob2 /DNDEBUG")
endif()
