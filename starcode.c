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

int starcode(...) {
    // Existing code...
    
    // Clean up before returning
    for (int i = 0; i < n_seqs; i++) {
        destroy_useq(seqs[i]);
    }
    destroy_lookup(lookup);
    
    return status;
} 