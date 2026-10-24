# Diff Coverage
## Diff: c6c4303d43cbf66209853b43804da08fbf15a3ce...HEAD, staged, unstaged and untracked changes

- bike-ui/components/SegmentBuilderPage&#46;tsx (100%)
- bike-ui/components/SegmentBuilderWorkspace&#46;tsx (75.0%): Missing lines 481

## Summary

- **Total**: 5 lines
- **Missing**: 1 line
- **Coverage**: 80.0%



## bike-ui/components/SegmentBuilderWorkspace&#46;tsx

```
  477                 onClick={() => {
  478                   void handleSave();
  479                 }}
  480               >
! 481                 {isSaving ? "Saving..." : "Save segment"}
  482               </button>
  483             </div>
  484           </div>
  485         </>
```


---


