cmake_minimum_required(VERSION 3.14)
set(PROJECT_NAME "veilid")
project(${PROJECT_NAME} LANGUAGES CXX)

# This value is used when generating builds using this plugin, so it must
# not be changed
set(PLUGIN_NAME "veilid_plugin")

add_library(${PLUGIN_NAME} SHARED
  "veilid_plugin.cpp"
)
apply_standard_settings(${PLUGIN_NAME})
set_target_properties(${PLUGIN_NAME} PROPERTIES
  CXX_VISIBILITY_PRESET hidden)
target_compile_definitions(${PLUGIN_NAME} PRIVATE FLUTTER_PLUGIN_IMPL)
target_include_directories(${PLUGIN_NAME} INTERFACE
  "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(${PLUGIN_NAME} PRIVATE flutter flutter_wrapper_plugin)

include(./rust.cmake)
# List of absolute paths to libraries that should be bundled with the plugin
set(veilid_bundled_libraries
  "$<TARGET_FILE:${CRATE_NAME}-shared>"
  PARENT_SCOPE
)