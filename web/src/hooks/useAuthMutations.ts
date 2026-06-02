'use client';

import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useAuth } from '@/lib/auth/auth-context';
import type { 
  LoginPayload, 
  SignupPayload, 
  PasswordResetPayload,
  PasswordResetConfirmPayload,
  MFASetupPayload,
  MFAVerifyPayload
} from '@/types/auth';

export function useLoginMutation() {
  const { login } = useAuth();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (payload: LoginPayload) => login(payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['user'] });
    },
  });
}

export function useSignupMutation() {
  const { signup } = useAuth();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (payload: SignupPayload) => signup(payload),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['user'] });
    },
  });
}

export function useLogoutMutation() {
  const { logout } = useAuth();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => logout(),
    onSuccess: () => {
      queryClient.clear();
    },
  });
}

export function usePasswordResetMutation() {
  const { resetPassword } = useAuth();

  return useMutation({
    mutationFn: (payload: PasswordResetPayload) => resetPassword(payload),
  });
}

export function usePasswordResetConfirmMutation() {
  const { confirmPasswordReset } = useAuth();

  return useMutation({
    mutationFn: (payload: PasswordResetConfirmPayload) => confirmPasswordReset(payload),
  });
}

export function useMFASetupMutation() {
  const { setupMFA } = useAuth();

  return useMutation({
    mutationFn: (payload: MFASetupPayload) => setupMFA(payload),
  });
}

export function useMFAVerifyMutation() {
  const { verifyMFA } = useAuth();

  return useMutation({
    mutationFn: (payload: MFAVerifyPayload) => verifyMFA(payload),
  });
}
