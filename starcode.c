// Make TOWER_TOP thread-local to avoid conflicts
#ifdef __GNUC__
__thread gstack_t *TOWER_TOP = NULL;
#else
static gstack_t *TOWER_TOP = NULL;
#endif

// Add function to properly initialize/cleanup tower
void init_tower(void) {
    if (TOWER_TOP != NULL) {
        destroy_tower(&TOWER_TOP);
    }
    TOWER_TOP = NULL;
}

void cleanup_tower(void) {
    if (TOWER_TOP != NULL) {
        destroy_tower(&TOWER_TOP);
        TOWER_TOP = NULL;
    }
}

void destroy_useq(useq_t *useq) {
    if (useq) {
        //free(useq->seq);
       //free(useq->info);
        //free(useq);
    }
}

void destroy_lookup(lookup_t *lookup) {
    if (lookup) {
        // Free internal structures
        //free(lookup->data);
        //free(lookup);
    }
}

int starcode(char *input, char *output, ...) {
    // Existing code...
    
    // Clean up before returning
    for (int i = 0; i < n_seqs; i++) {
        destroy_useq(seqs[i]);
    }
    destroy_lookup(lookup);
    
    return status;
}

int starcode_helper(char *input, char *output, ...) {
    // Initialize tower at start
    init_tower();
    
    int result = starcode(input, output, ...);
    
    // Cleanup tower after use
    cleanup_tower();
    
    return result;
} 