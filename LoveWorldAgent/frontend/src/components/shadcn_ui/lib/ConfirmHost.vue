<script setup>
import { computed } from 'vue'
import { Button } from '@/components/shadcn_ui/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/shadcn_ui/ui/dialog'
import { confirmState, resolveConfirm } from './useConfirm'
const open = computed({
    get: () => confirmState.open,
    set: (value) => {
        if (!value) resolveConfirm(false)
    },
})
</script>

<template>
    <Dialog v-model:open="open">
        <DialogContent class="max-w-sm z-[100]" overlay-class="z-[100]">
            <DialogHeader>
                <DialogTitle>{{ confirmState.title }}</DialogTitle>
                <DialogDescription v-if="confirmState.description">{{ confirmState.description }}</DialogDescription>
            </DialogHeader>
            <DialogFooter>
                <Button variant="outline" size="sm" @click="resolveConfirm(false)">{{ confirmState.cancelText }}</Button>
                <Button :variant="confirmState.destructive ? 'destructive' : 'default'" size="sm" @click="resolveConfirm(true)">
                    {{ confirmState.confirmText }}
                </Button>
            </DialogFooter>
        </DialogContent>
    </Dialog>
</template>
