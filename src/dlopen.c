#include <dlfcn.h>
#include <stdint.h>
#include <stdio.h>

#define ERROR_MEMORY_NOT_ENOUGH -23
#define ERROR_DYNAMIC_LOADING -24
#define RISCV_PGSIZE 4096
#define ROUNDUP(a, b) ((((a)-1) / (b) + 1) * (b))

int simulator_internal_dlopen2(const char* native_library_path,
                               const uint8_t* code, size_t length,
                               uint8_t* aligned_addr, size_t aligned_size,
                               void** handle, size_t* consumed_size) {
  /* TODO: parse ELF and consume proper pages */
  (void)code;
  (void)aligned_addr;
  size_t aligned_length = ROUNDUP(length, RISCV_PGSIZE);
  if (aligned_size < aligned_length) {
    return ERROR_MEMORY_NOT_ENOUGH;
  }
  *consumed_size = aligned_length;
  *handle = dlopen(native_library_path, RTLD_NOW);
  if (*handle == NULL) {
    printf("Error occurs in dlopen: %s\n", dlerror());
    return -1;
  }
  return 0;
}

void* ckb_dlsym(void* handle, const char* symbol) {
  return dlsym(handle, symbol);
}
