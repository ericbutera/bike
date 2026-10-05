import {
  useCallback,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";

type KeyedValue<T> = {
  key: string;
  value: T;
};

/** Keep editable state aligned with the source record that initialized it. */
export function useKeyedState<T>(
  key: string,
  initialValue: T,
): [T, Dispatch<SetStateAction<T>>] {
  const [stored, setStored] = useState<KeyedValue<T>>({
    key,
    value: initialValue,
  });
  const value = stored.key === key ? stored.value : initialValue;

  const setValue = useCallback<Dispatch<SetStateAction<T>>>(
    (nextValue) => {
      setStored((current) => {
        const currentValue = current.key === key ? current.value : initialValue;
        return {
          key,
          value:
            typeof nextValue === "function"
              ? (nextValue as (previous: T) => T)(currentValue)
              : nextValue,
        };
      });
    },
    [initialValue, key],
  );

  return [value, setValue];
}
