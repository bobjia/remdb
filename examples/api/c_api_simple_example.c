/*
 * RemDB Library Link Test
 */

#include <stdio.h>
#include "remdb.h"

int main() {
    printf("RemDB Library Link Test\n");
    printf("=======================\n\n");
    
    // Just test that we can reference RemDB functions
    printf("Testing RemDB function references...\n");
    
    // Declare function pointers to test linking
    typedef enum RemDbError (*InitFunc)(const RemDbConfig*, RemDbHandle*);
    InitFunc init_func = remdb_init_global;
    
    printf("Successfully referenced remdb_init_global function\n");
    printf("Library link test passed!\n");
    
    return 0;
}